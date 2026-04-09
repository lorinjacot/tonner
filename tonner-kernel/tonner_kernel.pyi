from uuid import UUID
from tonner import Context, TonnerWorld

def context() -> Context: ...
def root_window() -> Window: ...

class Window:
    id: UUID
    world: TonnerWorld
    
    def __init__(self, world: TonnerWorld) -> None: ...