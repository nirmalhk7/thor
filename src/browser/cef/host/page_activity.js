(function observePageActivity(report) {
    "use strict";

    const baseline = new WeakMap();
    const drafts = new Set();
    const playing = new Set();
    const observedMedia = new WeakSet();
    const calling = new Set();
    let protectedPage = false;
    let recheck;

    const listen = (type, callback) => addEventListener(type, callback, true);
    const firstTarget = event => event.composedPath()[0] ?? event.target;

    function control(event) {
        const target = firstTarget(event);
        if (target instanceof HTMLInputElement) {
            return /^(button|hidden|image|reset|submit)$/.test(target.type) ? null : target;
        }
        if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
            return target;
        }
        return target?.isContentEditable
            ? target.closest("[contenteditable]") ?? target
            : null;
    }

    function value(field) {
        if (field instanceof HTMLInputElement && /^(checkbox|radio)$/.test(field.type)) {
            return field.checked;
        }
        if (field instanceof HTMLSelectElement) {
            return Array.from(field.selectedOptions, option => option.index).join(",");
        }
        return field.isContentEditable ? field.textContent : field.value;
    }

    function defaultValue(field) {
        if (field instanceof HTMLInputElement && /^(checkbox|radio)$/.test(field.type)) {
            return field.defaultChecked;
        }
        return field.defaultValue;
    }

    function update() {
        for (const field of drafts) {
            if (!field.isConnected || value(field) === baseline.get(field)) drafts.delete(field);
        }
        for (const media of playing) {
            if (media.paused || media.ended) playing.delete(media);
        }
        for (const peer of calling) {
            if (!hasMediaCall(peer)) calling.delete(peer);
        }
        const active = drafts.size > 0 || playing.size > 0 || calling.size > 0;
        if (active !== protectedPage) {
            protectedPage = active;
            report(active);
        }
        if (active && recheck === undefined) {
            recheck = setTimeout(() => {
                recheck = undefined;
                update();
            }, 2000);
        } else if (!active && recheck !== undefined) {
            clearTimeout(recheck);
            recheck = undefined;
        }
    }

    function remember(event) {
        const field = event.isTrusted && control(event);
        if (field && !baseline.has(field)) baseline.set(field, value(field));
    }

    function edited(event) {
        const field = event.isTrusted && control(event);
        if (!field) return;
        if (!baseline.has(field)) baseline.set(field, defaultValue(field));
        drafts.add(field);
        update();
    }

    listen("pagehide", event => {
        if (event.target === document && protectedPage) report(false);
    });
    listen("pageshow", event => {
        if (event.target === document && protectedPage) report(true);
    });

    listen("focusin", remember);
    listen("beforeinput", remember);
    listen("input", edited);
    listen("change", edited);
    listen("reset", event => {
        queueMicrotask(() => {
            if (event.defaultPrevented) return;
            for (const field of drafts) {
                if (field.form === event.target) {
                    baseline.delete(field);
                    drafts.delete(field);
                }
            }
            update();
        });
    });

    function updateMedia(media) {
        if (!media.paused && !media.ended) playing.add(media);
        else playing.delete(media);
        update();
    }

    function watchMedia(media) {
        if (observedMedia.has(media)) return;
        observedMedia.add(media);
        for (const type of ["play", "playing", "pause", "ended", "emptied", "error"]) {
            media.addEventListener(type, () => updateMedia(media));
        }
    }

    listen("play", event => {
        const media = firstTarget(event);
        if (media instanceof HTMLMediaElement) {
            watchMedia(media);
            updateMedia(media);
        }
    });
    HTMLMediaElement.prototype.play = new Proxy(HTMLMediaElement.prototype.play, {
        apply(play, media, args) {
            const result = Reflect.apply(play, media, args);
            watchMedia(media);
            updateMedia(media);
            return result;
        },
    });

    function hasMediaCall(peer) {
        if (!["connecting", "connected", "disconnected"].includes(peer.connectionState)) return false;
        return peer.getTransceivers().some(({ currentDirection, direction, sender, receiver }) => {
            const flow = currentDirection ?? direction;
            return (flow?.includes("send") && sender?.track?.readyState === "live")
                || (flow?.includes("recv") && receiver?.track?.readyState === "live");
        });
    }

    if (typeof RTCPeerConnection === "function") {
        globalThis.RTCPeerConnection = new Proxy(RTCPeerConnection, {
            construct(Connection, args, constructor) {
                const peer = Reflect.construct(Connection, args, constructor);
                const changed = () => {
                    if (hasMediaCall(peer)) calling.add(peer);
                    else calling.delete(peer);
                    update();
                };
                for (const event of ["connectionstatechange", "signalingstatechange", "track"]) {
                    peer.addEventListener(event, changed);
                }
                return peer;
            },
        });
    }
})
