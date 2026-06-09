import math
import datetime
import entropie as ent
import matplotlib.pyplot as plt
from matplotlib.animation import FuncAnimation

state = ent.State()
solver = ent.Solver()
solver.substep_count = 10
dt = datetime.timedelta(milliseconds=10)

a = state.add_particle()
b = state.add_particle(mass=1.0, position=[1, 0, 0])
c = state.add_particle(mass=1.0, position=[2, 0, 0])
state.add_distance_constraint([a, b], 1)
state.add_distance_constraint([b, c], 1)

for body in [b, c]:
    state.add_force(body, [0, 0, -9.81])

def get_pos(body):
    pos = state.position(body)
    return pos[0], pos[2]

fig, ax = plt.subplots()
ax.set_xlim(-2.5, 2.5)
ax.set_ylim(-2.5, 0.5)
ax.set_xlabel("x")
ax.set_ylabel("z")

line, = ax.plot([], [], "-o", lw=2)

def update(frame):
    solver.simulate(state, dt)
    line.set_data(*zip(get_pos(a), get_pos(b), get_pos(c)))
    return line,

ani = FuncAnimation(fig, update, cache_frame_data=False, interval=10, blit=True)
plt.show()