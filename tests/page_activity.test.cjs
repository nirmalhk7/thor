const assert = require("node:assert/strict");
const fs = require("node:fs");
const test = require("node:test");
const vm = require("node:vm");
const path = require("node:path");

const source = fs.readFileSync(path.join(__dirname, "../src/browser/cef/host/page_activity.js"), "utf8");

function fixture() {
    const listeners = new Map();
    const timers = new Map();
    const changes = [];
    let nextTimer = 0;
    let requests = 0;
    class Input {
        type = "text";
        value = "";
        defaultValue = "";
        isConnected = true;
    }
    class Media extends EventTarget {
        paused = true;
        ended = false;
        play() {
            this.paused = false;
            return Promise.resolve();
        }
    }
    class Peer extends EventTarget {
        connectionState = "new";
        getTransceivers() {
            return [{ currentDirection: "sendrecv", receiver: { track: { readyState: "live" } } }];
        }
    }
    const scope = {
        HTMLInputElement: Input,
        HTMLTextAreaElement: class {},
        HTMLSelectElement: class {},
        HTMLMediaElement: Media,
        RTCPeerConnection: Peer,
        async fetch() { requests += 1; },
        addEventListener(type, callback) {
            if (!listeners.has(type)) listeners.set(type, []);
            listeners.get(type).push(callback);
        },
        setTimeout(callback) {
            timers.set(++nextTimer, callback);
            return nextTimer;
        },
        clearTimeout(id) { timers.delete(id); },
    };
    vm.runInNewContext(source, scope)(protectedPage => changes.push(protectedPage));
    return {
        changes, scope,
        requests: () => requests,
        event(type, target) {
            for (const callback of listeners.get(type) || []) {
                callback({ target, isTrusted: true, composedPath: () => [target] });
            }
        },
        tick() {
            const callbacks = [...timers.values()];
            timers.clear();
            callbacks.forEach(callback => callback());
        },
    };
}

test("a draft protects its page until the field is restored", () => {
    const f = fixture();
    const input = new f.scope.HTMLInputElement();
    f.event("beforeinput", input);
    input.value = "draft";
    f.event("input", input);
    assert.deepEqual(f.changes, [true]);
    input.value = "";
    f.tick();
    assert.deepEqual(f.changes, [true, false]);
});

test("detached audio protects its page until playback pauses", async () => {
    const f = fixture();
    const audio = new f.scope.HTMLMediaElement();
    await audio.play();
    assert.deepEqual(f.changes, [true]);
    audio.paused = true;
    audio.dispatchEvent(new Event("pause"));
    assert.deepEqual(f.changes, [true, false]);
});

test("a connected media call protects the page until it ends", () => {
    const f = fixture();
    const peer = new f.scope.RTCPeerConnection();
    peer.connectionState = "connected";
    peer.dispatchEvent(new Event("connectionstatechange"));
    assert.deepEqual(f.changes, [true]);
    peer.connectionState = "closed";
    peer.dispatchEvent(new Event("connectionstatechange"));
    assert.deepEqual(f.changes, [true, false]);
});

test("periodic network requests do not protect an idle page", async () => {
    const f = fixture();
    for (let index = 0; index < 3; index += 1) {
        await f.scope.fetch("/poll");
        f.tick();
    }
    assert.equal(f.requests(), 3);
    assert.deepEqual(f.changes, []);
});

test("a WebRTC connection without media does not protect a page", () => {
    const f = fixture();
    const peer = new f.scope.RTCPeerConnection();
    peer.getTransceivers = () => [];
    peer.connectionState = "connected";
    peer.dispatchEvent(new Event("connectionstatechange"));
    assert.deepEqual(f.changes, []);
});
