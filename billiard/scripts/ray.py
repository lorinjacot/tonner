import numpy as np

class Ray:
    def __init__(self, origin: np.ndarray, direction: np.ndarray):
        self.origin = origin
        self.direction = direction

    def intersects(self, ball) -> bool:
        # from https://en.wikipedia.org/wiki/Line%E2%80%93sphere_intersection
        u = self.direction
        o = self.origin
        c = ball.node.global_transformation @ np.array([0, 0, 0, 1])
        c = c[:3]
        r = ball.radius

        u_dot_oc = np.dot(u, o - c)
        grad = u_dot_oc ** 2 - (np.linalg.norm(o - c) ** 2 - r**2)
        if grad < 0.0:
            return False
        
        grad_root = np.sqrt(grad)
        d_plus = - u_dot_oc + grad_root
        return d_plus >= 0.0