import math
import datetime
import tonner
import matplotlib.pyplot as plt
from matplotlib.animation import FuncAnimation

g = 9.81
L = 1.0
t = 0.0

state = tonner.State()
solver = tonner.Solver()
solver.substep_count = 10
TIME_STEP = datetime.timedelta(milliseconds=10)

def theta1_ddot(m1, L1, theta1, theta1_dot, m2, L2, theta2, theta2_dot):
    return (
        - g * (2 * m1 + m2) * math.sin(theta1)
        - m2 * g * math.sin(theta1 - 2 * theta2)
        - 2 * math.sin(theta1 - theta2) * m2 * (
            theta2_dot**2 * L2 + theta1_dot**2 * L1 * math.cos(theta1 - theta2)
        )
    ) / (L1 * (2 * m1 + m2 - m2 * math.cos(2 * theta1 - 2 * theta2)))

def theta2_ddot(m1, L1, theta1, theta1_dot, m2, L2, theta2, theta2_dot):
    return (
        2 * math.sin(theta1 - theta2) * (
            theta1_dot**2 * L1 * (m1 + m2)
            + g * (m1 + m2) * math.cos(theta1)
            + theta2_dot**2 * L2 * m2 * math.cos(theta1 - theta2)
        )
    ) / (L2 * (2 * m1 + m2 - m2 * math.cos(2 * theta1 - 2 * theta2)))

a = state.add_particle()
b = state.add_particle(mass=1.0, position=[1, 0, 0])
c = state.add_particle(mass=1.0, position=[2, 0, 0])
state.add_particle_distance_constraint([a, b], 1)
state.add_particle_distance_constraint([b, c], 1)

m1 = 1.0
L1 = L
theta1 = math.pi / 2
theta1_dot = 0.0

m2 = 1.0
L2 = L
theta2 = math.pi / 2
theta2_dot = 0.0

for body in [b, c]:
    state.add_force(body, [0, 0, -9.81])

def get_pos(body):
    pos = state.position(body)
    return pos[0], pos[2]

def get_pos_analytical():
    x1 = L1 * math.sin(theta1)
    z1 = -L1 * math.cos(theta1)
    x2 = x1 + L2 * math.sin(theta2)
    z2 = z1 - L2 * math.cos(theta2)
    return (0, 0), (x1, z1), (x2, z2)

fig, ax = plt.subplots()
ax.set_aspect("equal")
ax.set_xlim(-2.5, 2.5)
ax.set_ylim(-2.5, 0.5)
ax.set_xlabel("x")
ax.set_ylabel("z")

line, = ax.plot([], [], "-o", lw=2, label="numerical")
analytical_line, = ax.plot([], [], "-o", lw=1, label="analytical")
ax.legend()

def update(frame):
    global t, theta1, theta1_dot, theta2, theta2_dot
    dt = TIME_STEP.total_seconds()
    t += dt

    # semi-implicit Euler integration for the analytical solution
    theta1_dot += theta1_ddot(m1, L1, theta1, theta1_dot, m2, L2, theta2, theta2_dot) * dt
    theta2_dot += theta2_ddot(m1, L1, theta1, theta1_dot, m2, L2, theta2, theta2_dot) * dt

    theta1 += theta1_dot * dt
    theta2 += theta2_dot * dt

    solver.simulate(state, TIME_STEP)
    line.set_data(*zip(get_pos(a), get_pos(b), get_pos(c)))
    analytical_line.set_data(*zip(*get_pos_analytical()))
    
    return line, analytical_line

ani = FuncAnimation(fig, update, cache_frame_data=False, interval=10, blit=True)
plt.show()