# Viewer proof status

- Wasm build: crates/viewer/build-web.sh (cargo release build for wasm32, then wasm-bindgen --target web, then wasm-opt -Oz). Exit 0. dist/viewer/READY exists.
- Browser run of dist/viewer: loads in headless Chrome (WebGL2 through ANGLE Metal) and renders the village and the match directory. See evidence-viewer/wasm-directory.png.
- Missing proof: no browser capture of the live and replay screens yet, because headless Chrome cannot click. Those screens and both tank models are shown by the native capture of the same code (evidence-viewer/*.jpg: directory, live orbit, follow, gunner, disconnect, lag, recovery, replay, missing interval, abort, result).
- Next step: open dist/viewer in a browser, click "Watch live" and "Replay", and capture each screen.
- Not done: the real contract feed is not connected. A contract v1 sample is rendered and labelled as a demo.
