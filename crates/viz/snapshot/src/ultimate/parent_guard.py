"""Stop this directly owned background Blender when its capture owner disappears."""
import os
import sys
import threading
import time


def install_parent_guard():
    expected = int(os.environ["TOSKLIGHT_CAPTURE_PARENT_PID"])
    if expected <= 0:
        raise RuntimeError("Invalid capture owner PID")

    def abandoned():
        try:
            print("ToskLight capture owner exited; stopping owned Blender job", file=sys.stderr, flush=True)
        finally:
            os._exit(84)

    if os.name == "nt":
        import ctypes
        from ctypes import wintypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        kernel.WaitForSingleObject.restype = wintypes.DWORD
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        handle = kernel.OpenProcess(0x00100000, False, expected)
        if not handle:
            abandoned()

        def watch():
            try:
                while kernel.WaitForSingleObject(handle, 200) == 258:
                    pass
                abandoned()
            finally:
                kernel.CloseHandle(handle)
    else:
        # Check immediately too: the helper can die between spawn and Python startup.
        if os.getppid() != expected:
            abandoned()

        def watch():
            while True:
                if os.getppid() != expected:
                    abandoned()
                time.sleep(0.2)

    # Cycles releases the GIL while rendering. This process-local guard does not kill any
    # unrelated Blender process; forced owner termination is the only abandonment signal.
    threading.Thread(target=watch, name="tosklight-capture-owner", daemon=True).start()
