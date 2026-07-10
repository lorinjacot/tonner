import datetime
import tonner
import math
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.patches as patches
from matplotlib.animation import FuncAnimation
from scipy.spatial.transform import Rotation

engine = tonner.Engine()
engine.substep_count = 10
dt = datetime.timedelta(milliseconds=20)

COMPLIANCE = 0
LINEAR_DAMPING = 0
ANGULAR_DAMPING = 0
DISTANCE = 3
LOCAL_POINT_A = [0, 0, 0.5]
LOCAL_POINT_B = [0.001, 0, -0.5]

a = engine.add_rigid_box(
    position=[0, 0, 0],
    mass=math.inf,
)

b = engine.add_rigid_box(
    position=[0, 0, -2],
    mass=1.0,
)
engine.add_force(b, [0, 0, -9.81])

c = engine.add_rigid_box(
    position=[0, 0, -4],
    mass=1.0,
)
engine.add_force(c, [0, 0, -9.81])

engine.add_attach_joint(
    bodies=[a, b],
    rest_distance=DISTANCE,
    attachment_points=[LOCAL_POINT_A, LOCAL_POINT_B],
    compliance=COMPLIANCE,
)

engine.add_attach_joint(
    bodies=[b, c],
    rest_distance=DISTANCE,
    attachment_points=[LOCAL_POINT_A, LOCAL_POINT_B],
    compliance=COMPLIANCE,
)

fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(8, 6))
ax1.set_aspect("equal")
ax1.set_xlim(-2, 2)
ax1.set_ylim(-10, 2)
ax1.set_xlabel("x")
ax1.set_ylabel("z")

ax2.set_xlim(0, 500)
ax2.set_ylim(0, 50)
ax2.set_xlabel("Time")
ax2.set_ylabel("Energy")

a_box = patches.Rectangle(
    (0, 0), 1, 1, rotation_point="center", color="blue"
)
ax1.add_patch(a_box)

b_box = patches.Rectangle(
    (0, 0), 1, 1, rotation_point="center", color="orange"
)
ax1.add_patch(b_box)

c_box = patches.Rectangle(
    (0, 0), 1, 1, rotation_point="center", color="green"
)
ax1.add_patch(c_box)

line_ab, = ax1.plot([], [], color="black", linewidth=1)
line_bc, = ax1.plot([], [], color="black", linewidth=1)

energy_history = []
energy_line, = ax2.plot([], [], color="red", linewidth=1)

def draw_box(body, rect_patch: patches.Rectangle):
    pos = engine.position(body)
    orientation = engine.orientation(body)

    xy = pos[0] - 0.5, pos[2] - 0.5
    rect_patch.set_xy(xy)

    r = Rotation.from_quat(orientation)
    # Using "yxz" puts the Y-rotation at index 0 with a full [-180, 180] range
    rect_patch.angle = -r.as_euler("yxz", degrees=True)[0]

def draw_line(body1, body2, line):
    pos1 = engine.position(body1)
    orientation1 = engine.orientation(body1)

    pos2 = engine.position(body2)
    orientation2 = engine.orientation(body2)
    
    dot1 = pos1 + Rotation.from_quat(orientation1).apply(LOCAL_POINT_A)
    dot2 = pos2 + Rotation.from_quat(orientation2).apply(LOCAL_POINT_B)

    line.set_data([dot1[0], dot2[0]], [dot1[2], dot2[2]])

def total_energy():
    kinetic = 0
    potential = 0

    for body in [a, b, c]:
        m = engine.mass(body)
        if m == math.inf:
            m = 0
        v = np.array(engine.velocity(body))
        kinetic += 0.5 * m * (v @ v)

        I = np.array(engine.inertia(body))
        q = engine.orientation(body)
        r = Rotation.from_quat(q)

        angular_velocity = r.apply(np.array(engine.angular_velocity(body)))
        kinetic += 0.5 * (angular_velocity @ I @ angular_velocity)

        z = engine.position(body)[2]
        potential += m * 9.81 * z

    return 100 + kinetic + potential

def update(frame):
    engine.simulate(dt)
    energy_history.append(total_energy())

    draw_box(a, a_box)
    draw_box(b, b_box)
    draw_box(c, c_box)

    draw_line(a, b, line_ab)
    draw_line(b, c, line_bc)

    energy_line.set_data(range(len(energy_history)), energy_history)
    total_frames = len(energy_history)
    if total_frames % 500 == 0:
        ax2.set_xlim(0, total_frames + 500)
        ax2.figure.canvas.draw_idle()

    # return a_box, b_box, line_ab
    return a_box, b_box, c_box, line_ab, line_bc, energy_line

ani = FuncAnimation(fig, update, cache_frame_data=False, interval=dt.microseconds // 1000, blit=True)
plt.show()