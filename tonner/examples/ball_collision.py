import tonner
import datetime
import matplotlib.pyplot as plt
from matplotlib.animation import FuncAnimation
from matplotlib.patches import Circle

state = tonner.State()
solver = tonner.Solver()
dt = datetime.timedelta(milliseconds=16)

RADIUS = 0.5

a = state.add_rigid_ball(
    position=[-1, 0, 0],
    velocity=[1, 0, 0],
    mass=1.0,
    radius=RADIUS
)
a_shape = Circle((0, 0), RADIUS, color="blue")

b = state.add_rigid_ball(
    position=[1, 0, 0],
    velocity=[-1, 0, 0],
    mass=1.0,
    radius=RADIUS
)
b_shape = Circle((0, 0), RADIUS, color="red")

fig, ax = plt.subplots(figsize=(6, 6))
ax.set_aspect("equal")
ax.set_xlim(-2, 2)
ax.set_ylim(-2, 2)

ax.add_patch(a_shape)
ax.add_patch(b_shape)

def draw_circle(shape, body_id):
    pos = state.position(body_id)
    shape.center = (pos[0], pos[1])

def update(frame):
    solver.simulate(state, dt)
    draw_circle(a_shape, a)
    draw_circle(b_shape, b)
    return a_shape, b_shape

ani = FuncAnimation(fig, update, cache_frame_data=False, blit=True, interval=dt.microseconds / 1000)
plt.show()