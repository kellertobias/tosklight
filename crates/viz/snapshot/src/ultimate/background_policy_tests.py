"""CPU-only managed capture scheduling tests. No Blender import or subprocess."""
import importlib.util
import os
import pathlib
import types
import unittest
from unittest.mock import patch

path = pathlib.Path(__file__).with_name("background_policy.py")
spec = importlib.util.spec_from_file_location("capture_background_policy", path)
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class BackgroundPolicyTests(unittest.TestCase):
    def test_factory_default_is_overridden_without_touching_render_quality(self):
        scene = types.SimpleNamespace(
            render=types.SimpleNamespace(threads_mode="AUTO", threads=128, resolution_x=3840,
                                         resolution_y=2160, resolution_percentage=100),
            cycles=types.SimpleNamespace(samples=256, volume_bounces=2, volume_step_rate=0.5,
                                         volume_max_steps=1024, transparent_max_bounces=16))
        before_render = vars(scene.render).copy()
        before_cycles = vars(scene.cycles).copy()
        with patch.dict(os.environ, {"TOSKLIGHT_CAPTURE_THREADS": "7"}, clear=True):
            policy.apply_background_policy(scene)
        self.assertEqual(scene.render.threads_mode, "FIXED")
        self.assertEqual(scene.render.threads, 7)
        after_render = vars(scene.render).copy()
        for key in ("threads_mode", "threads"):
            before_render.pop(key)
            after_render.pop(key)
        self.assertEqual(before_render, after_render)
        self.assertEqual(before_cycles, vars(scene.cycles))

    def test_invalid_or_missing_budget_cannot_silently_revert_to_auto(self):
        for value in (None, "0", "9", "-1", "NaN", "", "100000"):
            scene = types.SimpleNamespace(render=types.SimpleNamespace(threads_mode="AUTO", threads=128))
            with patch.dict(os.environ, {} if value is None else {"TOSKLIGHT_CAPTURE_THREADS": value}, clear=True):
                with self.assertRaises(RuntimeError):
                    policy.apply_background_policy(scene)
            self.assertEqual(vars(scene.render), {"threads_mode": "AUTO", "threads": 128})


if __name__ == "__main__":
    unittest.main()
