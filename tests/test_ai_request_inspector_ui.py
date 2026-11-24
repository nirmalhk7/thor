"""Run with Python Playwright and THOR_TEST_CHROMIUM set."""

import functools
import json
import os
import threading
import unittest
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


FIXTURE = {
    "model": "fixture-model",
    "messages": [
        {"role": "system", "content": "hello world"},
        {"role": "user", "content": "1234"},
    ],
    "stream": True,
    "temperature": 0.25,
}
SUMMARY = {
    "model": "fixture-model",
    "message_count": 2,
    "roles": ["system", "user"],
    "prompt_tokens": 5,
    "temperature": 0.25,
    "streaming": True,
}


@unittest.skipUnless(os.environ.get("THOR_TEST_CHROMIUM"), "No test Chromium supplied")
class AiRequestInspectorUiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from playwright.sync_api import expect, sync_playwright

        cls.expect = staticmethod(expect)
        root = Path(__file__).resolve().parents[1] / "src" / "workbench"
        server = ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(SimpleHTTPRequestHandler, directory=root)
        )
        cls.addClassCleanup(server.server_close)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        cls.addClassCleanup(server.shutdown)
        cls.origin = f"http://127.0.0.1:{server.server_port}"
        playwright = sync_playwright().start()
        cls.addClassCleanup(playwright.stop)
        cls.browser = playwright.chromium.launch(
            executable_path=os.environ["THOR_TEST_CHROMIUM"]
        )
        cls.addClassCleanup(cls.browser.close)

    def setUp(self):
        self.context = self.browser.new_context()
        self.addCleanup(self.context.close)
        self.page = self.context.new_page()
        self.page.add_init_script(
            """
            window.cefQuery = ({request, onSuccess, onFailure}) => {
                window.command = JSON.parse(request);
                window.finish = (result) => onSuccess(JSON.stringify(result));
                window.fail = (message) => onFailure(400, message);
            };
            """
        )
        self.page.goto(f"{self.origin}/ai.html")

    def test_inspect_and_copy_only_the_summary(self):
        self.context.grant_permissions(["clipboard-read", "clipboard-write"], origin=self.origin)
        self.page.get_by_label("Request body (JSON)").fill(json.dumps(FIXTURE))
        self.page.get_by_role("button", name="Inspect request").click()
        self.assertEqual(self.page.evaluate("window.command"), {
            "command": "ai_inspect_request",
            "body": FIXTURE,
        })
        self.page.evaluate("(result) => window.finish(result)", SUMMARY)
        self.expect(self.page.get_by_role("status")).to_have_text("Summary ready")
        self.page.get_by_role("button", name="Copy summary").click()
        self.expect(self.page.get_by_role("status")).to_have_text("Summary copied")
        clipboard = self.page.evaluate("navigator.clipboard.readText()")
        self.assertEqual(json.loads(clipboard), SUMMARY)
        self.assertNotIn("hello world", clipboard)

    def test_changed_input_cannot_export_stale_result_and_invalid_json_stays_local(self):
        body = self.page.get_by_label("Request body (JSON)")
        inspect = self.page.get_by_role("button", name="Inspect request")
        body.fill(json.dumps(FIXTURE))
        inspect.click()
        body.fill('{"model":"changed","prompt":"short"}')
        self.page.evaluate("(result) => window.finish(result)", SUMMARY)
        self.expect(self.page.get_by_role("status")).to_have_text("Ready")
        self.expect(self.page.get_by_role("button", name="Copy summary")).to_be_hidden()
        self.expect(self.page.locator("#result")).to_be_empty()
        self.page.evaluate("window.command = null")
        body.fill("{invalid}")
        inspect.click()
        self.expect(self.page.get_by_role("status")).to_contain_text("Enter valid JSON")
        self.assertIsNone(self.page.evaluate("window.command"))
