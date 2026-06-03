import math
import datetime
import numpy.typing as npt

Vec3 = npt.ArrayLike

class BodyId:
    pass

class State:
    def __init__(self):
        pass

    def add_particle(
        self,
        position: Vec3 = [0, 0, 0],
        velocity: Vec3 = [0, 0, 0],
        mass: float = math.inf
    ) -> BodyId:
        pass

    def position(self, body: BodyId) -> list[float]:
        pass

    def add_force(self, body: BodyId, force: Vec3):
        pass

class Solver:
    def __init__(self):
        pass

    def simulate(self, state: State, delta_time: datetime.timedelta):
        pass