import sys

from ipykernel.kernelbase import Kernel

class TonnerKernel(Kernel):
    implementation = "Tonner"
    implementation_version = "0.0.1"
    banner = "Tonner Kernel - A jupyter kernel for Tonner"

    language_info = {
        "name": "python",
        "version": sys.version,
        "mimetype": "text/x-python",
        "file_extension": ".py"
    }
    
    def do_execute(self, code, silent, store_history=True, user_expressions=None, allow_stdin=False, *, cell_meta=None, cell_id=None):
        if not silent:
            stream_content = {'name': 'stdout', 'text': code}
            self.send_response(self.iopub_socket, 'stream', stream_content)

        return {'status': 'ok',
                # The base class increments the execution count
                'execution_count': self.execution_count,
                'payload': [],
                'user_expressions': {},
               }