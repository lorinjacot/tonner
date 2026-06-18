import datetime
import tonner
import math
import matplotlib.pyplot as plt
import matplotlib.patches as patches
from matplotlib.animation import FuncAnimation
from scipy.spatial.transform import Rotation

state = tonner.State()
solver = tonner.Solver()
solver.substep_count = 1000
dt = datetime.timedelta(milliseconds=10)

COMPLIANCE = 0.01
LINEAR_DAMPING = 0
ANGULAR_DAMPING = 0
DISTANCE = 1
LOCAL_POINT_A = [0, 0, 0]
LOCAL_POINT_B = [0.5, 0, 0.5]

a = state.add_rigid_box(
    position=[0, 0, 0],
    mass=math.inf,
)

b = state.add_rigid_box(
    position=[0, 0, -2],
    mass=1.0,
)
state.add_force(b, [0, 0, -9.81])

c = state.add_rigid_box(
    position=[0, 0, -4],
    mass=1.0,
)
state.add_force(c, [0, 0, -9.81])

state.add_distance_constraint(
    bodies=[a, b],
    distance=DISTANCE,
    compliance=COMPLIANCE,
    linear_damping=LINEAR_DAMPING,
    angular_damping=ANGULAR_DAMPING,
    application_points=[LOCAL_POINT_A, LOCAL_POINT_B],
)

state.add_distance_constraint(
    bodies=[b, c],
    distance=DISTANCE,
    compliance=COMPLIANCE,
    linear_damping=LINEAR_DAMPING,
    angular_damping=ANGULAR_DAMPING,
    application_points=[LOCAL_POINT_A, LOCAL_POINT_B],
)

fig, ax = plt.subplots()
ax.set_aspect("equal")
ax.set_xlim(-2, 2)
ax.set_ylim(-10, 2)
ax.set_xlabel("x")
ax.set_ylabel("z")

a_box = patches.Rectangle(
    (0, 0), 1, 1, rotation_point="center", color="blue"
)
ax.add_patch(a_box)

b_box = patches.Rectangle(
    (0, 0), 1, 1, rotation_point="center", color="orange"
)
ax.add_patch(b_box)

c_box = patches.Rectangle(
    (0, 0), 1, 1, rotation_point="center", color="green"
)
ax.add_patch(c_box)

line_ab, = ax.plot([], [], color="black", linewidth=1)
line_bc, = ax.plot([], [], color="black", linewidth=1)

def draw_box(body, rect_patch: patches.Rectangle):
    pos = state.position(body)
    orientation = state.orientation(body)

    xy = pos[0] - 0.5, pos[2] - 0.5
    rect_patch.set_xy(xy)

    r = Rotation.from_quat(orientation)
    # Using "yxz" puts the Y-rotation at index 0 with a full [-180, 180] range
    rect_patch.angle = -r.as_euler("yxz", degrees=True)[0]

def draw_line(body1, body2, line):
    pos1 = state.position(body1)
    orientation1 = state.orientation(body1)

    pos2 = state.position(body2)
    orientation2 = state.orientation(body2)
    
    dot1 = pos1 + Rotation.from_quat(orientation1).apply(LOCAL_POINT_A)
    dot2 = pos2 + Rotation.from_quat(orientation2).apply(LOCAL_POINT_B)

    line.set_data([dot1[0], dot2[0]], [dot1[2], dot2[2]])

def update(frame):
    solver.simulate(state, dt)

    draw_box(a, a_box)
    draw_box(b, b_box)
    draw_box(c, c_box)

    draw_line(a, b, line_ab)
    draw_line(b, c, line_bc)

    # return a_box, b_box, line_ab
    return a_box, b_box, c_box, line_ab, line_bc

ani = FuncAnimation(fig, update, cache_frame_data=False, interval=10, blit=True)
plt.show()