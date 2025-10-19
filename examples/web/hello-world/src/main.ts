import { Engine, Scene } from "../../../../storm/pkg"

let engine = await Engine.builder().build();
let scene = Scene.builder().build(engine);