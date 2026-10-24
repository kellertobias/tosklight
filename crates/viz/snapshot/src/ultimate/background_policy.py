"""Managed image CPU budget, reapplied after factory initialization. No quality controls."""
import os


def apply_background_policy(scene):
    try:
        threads = int(os.environ["TOSKLIGHT_CAPTURE_THREADS"])
    except (KeyError, ValueError) as error:
        raise RuntimeError("Managed image CPU thread budget is missing or invalid") from error
    if not 1 <= threads <= 8:
        raise RuntimeError("Managed image CPU thread budget must be between one and eight")
    scene.render.threads_mode = "FIXED"
    scene.render.threads = threads
    print("TOSKLIGHT_CAPTURE_POLICY fixed_cpu_threads=" + str(threads), flush=True)
