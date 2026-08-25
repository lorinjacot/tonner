import numpy as np

class Ray:
    def __init__(self, origin: np.ndarray, direction: np.ndarray):
        self.origin = origin
        self.direction = direction

    def intersects_ball(self, ball) -> bool:
        # from https://en.wikipedia.org/wiki/Line%E2%80%93sphere_intersection
        u = self.direction
        o = self.origin
        c = ball.position
        c = c[:3]
        r = ball.radius

        u_dot_oc = np.dot(u, o - c)
        grad = u_dot_oc ** 2 - (np.linalg.norm(o - c) ** 2 - r**2)
        if grad < 0.0:
            return False
        
        grad_root = np.sqrt(grad)
        d_plus = - u_dot_oc + grad_root
        return d_plus >= 0.0
    
    def intersection_table(self) -> np.ndarray | None:
        # from https://en.wikipedia.org/wiki/Line%E2%80%93plane_intersection
        n = np.array([0, 1, 0])
        p0 = np.zeros(3)

        l0 = self.origin
        l = self.direction

        l_dot_n = np.dot(l, n)
        if l_dot_n == 0:
            return None
        d = np.dot(p0 - l0, n) / l_dot_n

        return l0 + l * d