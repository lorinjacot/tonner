class Point:
    def __init__(self, value: float, tangent: float):
        self.value = value
        self.tangent = tangent

def cubic_hermite_spline(t: float, p0: Point, p1: Point) -> float:
    return (
        (2*t**3 - 3*t**2 + 1) * p0.value
        + (t**3 - 2*t**2 + t) * p0.tangent
        + (-2*t**3 + 3*t**2) * p1.value
        + (t**3 - t**2) * p1.tangent
    )