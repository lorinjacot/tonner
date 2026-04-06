from ipykernel.kernelapp import IPKernelApp
from .kernel import TonnerKernel

IPKernelApp.launch_instance(kernel_class=TonnerKernel)