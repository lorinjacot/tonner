from numpy.typing import NDArray

class Ball:
    color: BallColor
    radius: float
    out: bool
    number: int
    position: NDArray
    velocity: NDArray

class BallColor:
    White: BallColor
    SolidYellow: BallColor
    SolidBlue: BallColor
    SolidRed: BallColor
    SolidPurple: BallColor
    SolidOrange: BallColor
    SolidGreen: BallColor
    SolidMaroon: BallColor
    Black: BallColor
    YellowStripe: BallColor
    BlueStripe : BallColor
    RedStripe : BallColor
    PurpleStripe : BallColor
    OrangeStripe : BallColor
    GreenStripe : BallColor
    MaroonStripe : BallColor