import { Engine, Scene } from "storm-js"

let engine = await Engine.builder().build();
let scene = Scene.builder().build(engine);