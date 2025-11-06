import { CameraBuilder, EngineBuilder, NodeBuilder, SceneBuilder, Vec3 } from "storm-js"

let canvas = document.querySelector("canvas");
if (canvas == null) { throw "Failed to get html canvas element" }

let engine = await new EngineBuilder().build();
let surface = engine.createSurfaceFromCanvasElement(canvas);

let scene = await new SceneBuilder().build(engine);

let cameraNode = new NodeBuilder().name("camera").translation(new Vec3(0.0, 0.0, 0.0)).build(scene)
let camera = new CameraBuilder().node(cameraNode).perspective({
    aspectRatio: 1.0,
    yfov: 60.0 * Math.PI / 180.0,
    zfar: 10.0,
    znear: 0.1,
}).build(scene)

let start = undefined as DOMHighResTimeStamp | undefined;

function step(timestamp: DOMHighResTimeStamp) {
    if (start === undefined) {
        start = timestamp;
    }
    const elapsed = timestamp - start;

    scene.simulate(elapsed);
    scene.render(surface, camera)

    requestAnimationFrame(step)
}

requestAnimationFrame(step)