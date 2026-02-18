import debugpy

debugpy.listen(5678, in_process_debug_adapter=True)


def update():
    print("Hello, world!")
