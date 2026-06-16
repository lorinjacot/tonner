import datetime
import entropie as ent
import matplotlib.pyplot as plt

state = ent.State()

a = state.add_particle(mass=1.0)
b = state.add_particle(mass=1.0, velocity=[0, 0, 10])

solver = ent.Solver()
dt = datetime.timedelta(milliseconds=10)

for body in [a, b]:
    state.add_force(body, [0, 0, -10])

pos_a = []
pos_b = []
for _ in range(200):
    solver.simulate(state, dt)
    pos_a.append(state.position(a)[2])
    pos_b.append(state.position(b)[2])

plt.plot(pos_a, label="a")
plt.plot(pos_b, label="b")
plt.grid()
plt.legend()
plt.show()