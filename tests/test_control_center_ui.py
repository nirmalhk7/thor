"""Embedded UI smoke tests. Run with Python Playwright and THOR_TEST_CHROMIUM set."""

import functools
import os
import threading
import unittest
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


@unittest.skipUnless(os.environ.get("THOR_TEST_CHROMIUM"), "No test Chromium supplied")
class ControlCenterUiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from playwright.sync_api import expect, sync_playwright

        cls.expect = staticmethod(expect)
        root = Path(__file__).resolve().parents[1] / "src" / "workbench"
        server = ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(QuietHandler, directory=root)
        )
        cls.addClassCleanup(server.server_close)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        cls.addClassCleanup(server.shutdown)
        cls.url = f"http://127.0.0.1:{server.server_port}/tools.html"
        playwright = sync_playwright().start()
        cls.addClassCleanup(playwright.stop)
        cls.browser = playwright.chromium.launch(
            executable_path=os.environ["THOR_TEST_CHROMIUM"]
        )
        cls.addClassCleanup(cls.browser.close)

    def setUp(self):
        self.page = self.browser.new_page()
        self.addCleanup(self.page.close)
        self.page.add_init_script(
            """
            window.invocations = [];
            window.cefQuery = ({request, onSuccess, onFailure}) => {
                const command = JSON.parse(request);
                if (command.command === "feature_list") {
                    onSuccess(JSON.stringify([
                        {name:"First tool", slug:"first", category:"QA", enabled:true, status:"Ready"},
                        {name:"Second tool", slug:"second", category:"QA", enabled:true, status:"Ready"}
                    ]));
                    return;
                }
                window.invocations.push(command);
                window.finish = (success = true) => success
                    ? onSuccess(JSON.stringify({origin: command.slug, payload: command.payload}))
                    : onFailure(-1, "First tool failed");
            };
            """
        )
        self.page.goto(self.url)

    def pick(self, name):
        return self.page.get_by_role("button", name=f"{name} QA · Ready")

    def test_keyboard_selection_keeps_focus_and_reselection_keeps_input(self):
        first = self.pick("First tool")
        second = self.pick("Second tool")
        self.page.get_by_label("Find a feature").focus()
        self.page.keyboard.press("Tab")
        self.expect(first).to_be_focused()
        self.page.keyboard.press("Enter")
        self.expect(first).to_be_focused()
        self.expect(first).to_have_attribute("aria-pressed", "true")
        self.page.get_by_label("Input JSON").fill('{"draft":true}')
        first.focus()
        self.page.keyboard.press("Space")
        self.expect(self.page.get_by_label("Input JSON")).to_have_value('{"draft":true}')
        self.page.keyboard.press("Tab")
        self.expect(second).to_be_focused()
        self.page.keyboard.press("Space")
        self.expect(second).to_be_focused()
        self.expect(second).to_have_attribute("aria-pressed", "true")
        self.expect(first).to_have_attribute("aria-pressed", "false")

    def test_inflight_result_stays_with_its_original_selection(self):
        result = self.page.locator("#feature-result")
        status = self.page.locator("#feature-status")
        run = self.page.get_by_role("button", name="Run feature")
        self.pick("First tool").click()
        run.click()
        self.page.evaluate("location.hash = 'second'")
        self.expect(self.page.get_by_role("heading", name="Second tool")).to_be_visible()
        self.expect(run).to_be_disabled()
        payload = self.page.get_by_label("Input JSON")
        payload.fill('{"current":true}')
        self.page.evaluate("window.finish()")
        self.expect(status).to_have_text("Ready")
        self.expect(result).to_be_empty()
        self.expect(payload).to_have_value('{"current":true}')
        self.expect(run).to_be_enabled()
        run.click()
        self.page.evaluate("window.finish()")
        self.expect(result).to_contain_text('"origin": "second"')
        self.expect(result).to_contain_text('"current": true')

    def test_switching_away_and_back_still_discards_the_old_failure(self):
        status = self.page.locator("#feature-status")
        run = self.page.get_by_role("button", name="Run feature")
        self.pick("First tool").click()
        run.click()
        self.pick("Second tool").click()
        self.pick("First tool").click()
        self.page.evaluate("window.finish(false)")
        self.expect(status).to_have_text("Ready")
        self.expect(status).to_have_attribute("data-error", "false")
        self.expect(run).to_be_enabled()

    def test_invalid_json_explains_how_to_fix_it_without_calling_rust(self):
        self.pick("First tool").click()
        self.page.get_by_label("Input JSON").fill("{draft: true}")
        self.page.get_by_role("button", name="Run feature").click()
        self.expect(self.page.locator("#feature-status")).to_contain_text(
            "Property names and text values must use double quotes."
        )
        self.assertEqual(self.page.evaluate("window.invocations"), [])
