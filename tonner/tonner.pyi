import math
import datetime

Vec3 = list[float] | tuple[float, float, float]
Quat = list[float] | tuple[float, float, float, float]
Mat3 = list[list[float]] | tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]

class BodyId:
    pass

class ParticleDistanceConstraintId:
    pass

class AttachJointId:
    pass

class Engine:
    substep_count: int = 10

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
        orientation: Quat = [0, 0, 0, 1],
        angular_velocity: Vec3 = [0, 0, 0],
        inertia: Mat3 = [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        radius: float = 1.0
    ) -> BodyId:
        pass

    def add_rigid_box(
        self,
        position: Vec3 = [0, 0, 0],
        velocity: Vec3 = [0, 0, 0],
        mass: float = math.inf,
        orientation: Quat = [0, 0, 0, 1],
        angular_velocity: Vec3 = [0, 0, 0],
        inertia: Mat3 = [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        dimensions: Vec3 = [1, 1, 1]
    ) -> BodyId:
        pass
    
    def add_particle_distance_constraint(
        self,
        particles: list[BodyId],
        distance: float = 0.0,
        compliance: float = 0.0,
    ) -> ParticleDistanceConstraintId:
        pass

    def add_attach_joint(
        self,
        bodies: list[BodyId] | tuple[BodyId, BodyId],
        rest_distance: float = 0.0,
        attachment_points: list[Vec3] | tuple[Vec3, Vec3] = [[0, 0, 0], [0, 0, 0]],
        compliance: float = 0.0,
    ) -> AttachJointId:
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

    def simulate(self, delta_time: datetime.timedelta) -> None:
        pass