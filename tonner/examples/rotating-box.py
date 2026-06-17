import datetime
import tonner
import matplotlib.pyplot as plt
import matplotlib.patches as patches
from matplotlib.animation import FuncAnimation
from scipy.spatial.transform import Rotation

state = tonner.State()
solver = tonner.Solver()
dt = datetime.timedelta(milliseconds=10)

width = 1.0
height = 2.0
depth = 1.0

box = state.add_rigid_box(
    angular_velocity=[0, 0, 1],
    dimensions=[width, height, depth]
)

fig, ax = plt.subplots()
ax.set_aspect("equal")
ax.set_xlim(-2, 2)
ax.set_ylim(-2, 2)
ax.set_xlabel("x")
ax.set_ylabel("y")

xy = state.position(box)[:2]
xy = xy[0] - width / 2, xy[1] - height / 2
rect_patch = patches.Rectangle(xy, width, height, rotation_point="center")

def update(frame):
    solver.simulate(state, dt)
    pos = state.position(box)[:2]
    orientation = state.orientation(box)

    xy = pos[0] - width / 2, pos[1] - height / 2
    rect_patch.set_xy(xy)
    r = Rotation.from_quat(orientation)
    rect_patch.angle = r.as_euler("xyz", degrees=True)[2]

    return rect_patch,

ax.add_patch(rect_patch)
ani = FuncAnimation(fig, update, cache_frame_data=False, interval=10, blit=True)
plt.show()