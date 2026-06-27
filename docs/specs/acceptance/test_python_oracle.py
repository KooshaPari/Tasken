"""Acceptance Test Oracle — Python SDK (FR-17)

This module encodes every Functional Requirement that touches the Python
SDK surface as a pending/ignored test skeleton.

These are the **asymptote**: they describe the desired behaviour derived
from the actual source code but are marked with @unittest.skip so the
suite runs without error while clearly showing what must pass before the
spec is considered complete.

Run:  python -m pytest docs/specs/acceptance/test_python_oracle.py -v -rs
"""

import unittest


class TestTaskenPythonOracle(unittest.TestCase):
    """Acceptance tests for the Python SDK.

    Each test maps to an FR-* requirement from docs/specs/SPEC.md.
    """

    # ------------------------------------------------------------------
    # FR-17: Python SDK / Bindings
    # ------------------------------------------------------------------

    @unittest.skip("FR-17.1: tasken.run() dispatches a task")
    def test_fr17_run_task(self):
        """tasken.run('build') shall dispatch the task and return a result."""
        # Given a Python environment with the tasken package installed
        # When  tasken.run('build') is called
        # Then  it shall dispatch the task
        # And   return a result dict with "exit_code" and "output"
        raise NotImplementedError(
            "FR-17.1: tasken.run('build') dispatch + result dict"
        )

    @unittest.skip("FR-17.2: tasken.list_tasks() returns task list")
    def test_fr17_list_tasks(self):
        """tasken.list_tasks() shall return a list of task definitions."""
        # Given tasks are registered
        # When  tasken.list_tasks() is called
        # Then  a list of task definitions shall be returned
        raise NotImplementedError(
            "FR-17.2: tasken.list_tasks() returns task definitions"
        )

    @unittest.skip("FR-17.3: tasken.schedule() registers a cron schedule")
    def test_fr17_schedule(self):
        """tasken.schedule(name, cron_expr) registers a recurring schedule."""
        # Given a tasken SDK instance
        # When  tasken.schedule('daily', '0 9 * * *', 'build') is called
        # Then  the schedule shall be registered in the scheduler
        raise NotImplementedError(
            "FR-17.3: tasken.schedule() registers a cron schedule"
        )

    @unittest.skip("FR-17.4: tasken.export() returns serialized tasks")
    def test_fr17_export(self):
        """tasken.export(fmt) shall return tasks in the requested format."""
        # Given tasks are registered
        # When  tasken.export('json') is called
        # Then  valid JSON containing task definitions shall be returned
        raise NotImplementedError(
            "FR-17.4: tasken.export('json') returns JSON string"
        )


if __name__ == "__main__":
    unittest.main()
