from typing import Type
from uuid import UUID
from abc import ABC, abstractmethod

class Context(WorldField):
    @classmethod
    def id(cls) -> UUID: ...

class World:
    def __init__(self, ctx: Context) -> None: ...

    def get[T: WorldField](self, type_: Type[T]) -> T | None: ...

class WorldField(ABC):
    @classmethod
    @abstractmethod
    def id(cls) -> UUID: ...

class Entity:
    def __init__(self, world: World) -> None: ...
