import math
import datetime
import numpy.typing as npt

Vec3 = npt.ArrayLike

class BodyId:
    pass

class PositionalConstraintId:
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

    def add_distance_constraint(
        self,
        bodies: list[BodyId] | tuple[BodyId, BodyId],
        distance: float,
        compliance: float = 0.0,
        application_points: npt.ArrayLike = [[0, 0, 0], [0, 0, 0]],
    ) -> PositionalConstraintId:
        pass

    def position(self, body: BodyId) -> list[float]:
        pass

    def add_force(self, body: BodyId, force: Vec3):
        pass

class Solver:
    substep_count: int = 10

    def __init__(self):
        pass

    def simulate(self, state: State, delta_time: datetime.timedelta):
        pass