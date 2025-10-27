import { EngineBuilder, SceneBuilder } from "storm-js"

let canvas = document.querySelector("canvas");
if (canvas == null) { throw "Failed to get html canvas element" }

let engine = await new EngineBuilder().build();
let surface = engine.createSurfaceFromCanvasElement(canvas);

let scene = await new SceneBuilder().build(engine);
let start = undefined as DOMHighResTimeStamp | undefined;

function step(timestamp: DOMHighResTimeStamp) {
    if (start === undefined) {
        start = timestamp;
    }
    const elapsed = timestamp - start;

    scene.simulate(elapsed);

    requestAnimationFrame(step)
}

requestAnimationFrame(step)