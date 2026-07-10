import datetime
import tonner
import matplotlib.pyplot as plt

engine = tonner.Engine()

a = engine.add_particle(mass=1.0)
b = engine.add_particle(mass=1.0, velocity=[0, 0, 10])

dt = datetime.timedelta(milliseconds=10)

for body in [a, b]:
    engine.add_force(body, [0, 0, -10])

pos_a = []
pos_b = []
for _ in range(200):
    engine.simulate(dt)
    pos_a.append(engine.position(a)[2])
    pos_b.append(engine.position(b)[2])

plt.plot(pos_a, label="a")
plt.plot(pos_b, label="b")
plt.grid()
plt.legend()
plt.show()