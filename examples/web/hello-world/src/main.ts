import { Engine, Scene } from "storm-js"

let engine = await Engine.builder().build();
let scene = await Scene.builder().build(engine);