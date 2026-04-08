use tonner::Context;

pub struct State {
    _ctx: Context,
    _surface: wgpu::Surface<'static>,
}

impl State {
    pub fn new(ctx: Context, surface: wgpu::Surface<'static>) -> State {
        State {
            _ctx: ctx,
            _surface: surface,
        }
    }
}
