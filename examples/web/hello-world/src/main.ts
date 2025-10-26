import { Engine, Scene } from "storm-js"

let canvas = document.querySelector("canvas");
if (canvas == null) { throw "Failed to get html canvas element" }

let engine = await Engine.builder().build();
let surface = engine.createSurfaceFromCanvasElement(canvas);

let scene = await Scene.builder().build(engine);
