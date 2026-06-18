import math
import datetime
import numpy.typing as npt

Vec3 = npt.ArrayLike

class BodyId:
    pass

class PositionalConstraintId:
    pass

class State:
    def __init__(self) -> None:
        pass

    def add_particle(
        self,
        position: Vec3 = [0, 0, 0],
        velocity: Vec3 = [0, 0, 0],
        mass: float = math.inf
    ) -> BodyId:
        pass

    def add_rigid_ball(
        self,
        position: Vec3 = [0, 0, 0],
        velocity: Vec3 = [0, 0, 0],
        mass: float = math.inf,
        orientation: Vec3 = [0, 0, 0, 1],
        angular_velocity: Vec3 = [0, 0, 0],
        inertia: npt.ArrayLike = [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        radius: float = 1.0
    ) -> BodyId:
        pass

    def add_rigid_box(
        self,
        position: Vec3 = [0, 0, 0],
        velocity: Vec3 = [0, 0, 0],
        mass: float = math.inf,
        orientation: Vec3 = [0, 0, 0, 1],
        angular_velocity: Vec3 = [0, 0, 0],
        inertia: npt.ArrayLike = [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        dimensions: Vec3 = [1, 1, 1]
    ) -> BodyId:
        pass

    def add_distance_constraint(
        self,
        bodies: list[BodyId] | tuple[BodyId, BodyId],
        distance: float,
        compliance: float = 0.0,
        linear_damping: float = 0.0,
        angular_damping: float = 0.0,
        application_points: npt.ArrayLike = [[0, 0, 0], [0, 0, 0]],
    ) -> PositionalConstraintId:
        pass

    def position(self, body: BodyId) -> list[float]:
        pass

    def velocity(self, body: BodyId) -> list[float]:
        pass

    def mass(self, body: BodyId) -> float:
        pass

    def orientation(self, body: BodyId) -> list[float]:
        pass

    def angular_velocity(self, body: BodyId) -> list[float]:
        pass

    def inertia(self, body: BodyId) -> list[list[float]]:
        pass

    def add_force(self, body: BodyId, force: Vec3) -> None:
        pass

class Solver:
    substep_count: int = 10

    def __init__(self) -> None:
        pass

    def simulate(self, state: State, delta_time: datetime.timedelta) -> None:
        pass