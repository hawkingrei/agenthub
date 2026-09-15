#!/usr/bin/env python3
"""Executable design experiment; this does not exercise the production runtime."""

import sqlite3
import tempfile
import unittest
from pathlib import Path


class FakeProcess:
    def __init__(self):
        self.alive = True

    def stop(self):
        self.alive = False


class ContractStore:
    def __init__(self, path):
        self.db = sqlite3.connect(path)
        self.db.executescript("""
            CREATE TABLE IF NOT EXISTS work (
                id TEXT PRIMARY KEY,
                actor TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'pending',
                generation INTEGER NOT NULL DEFAULT 0,
                outcome TEXT
            );
            CREATE TABLE IF NOT EXISTS reservations (
                actor TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                generation INTEGER NOT NULL,
                expires INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS generations (
                actor TEXT PRIMARY KEY,
                value INTEGER NOT NULL
            );
        """)

    def accept(self, work_id, actor="worker"):
        with self.db:
            self.db.execute(
                "INSERT OR IGNORE INTO work(id, actor) VALUES (?, ?)",
                (work_id, actor),
            )

    def claim(self, work_id, now=100):
        with self.db:
            self.db.execute("BEGIN IMMEDIATE")
            row = self.db.execute(
                "SELECT actor FROM work WHERE id = ? AND state = 'pending'",
                (work_id,),
            ).fetchone()
            if row is None:
                return None
            actor = row[0]
            if self.db.execute(
                "SELECT 1 FROM reservations WHERE actor = ?", (actor,)
            ).fetchone():
                return None
            generation = self.db.execute(
                "INSERT INTO generations VALUES (?, 1) ON CONFLICT(actor) "
                "DO UPDATE SET value = value + 1 RETURNING value",
                (actor,),
            ).fetchone()[0]
            self.db.execute(
                "INSERT INTO reservations VALUES (?, ?, ?, ?)",
                (actor, work_id, generation, now + 60),
            )
            self.db.execute(
                "UPDATE work SET state = 'running', generation = ? WHERE id = ?",
                (generation, work_id),
            )
            return generation

    def finish(self, work_id, generation, continuation=None, fail=False):
        with self.db:
            self.db.execute("BEGIN IMMEDIATE")
            changed = self.db.execute(
                "UPDATE work SET state = 'finalizing', outcome = 'progress' "
                "WHERE id = ? AND generation = ? AND state = 'running' "
                "AND EXISTS (SELECT 1 FROM reservations WHERE work_id = work.id "
                "AND generation = ? AND expires > 100)",
                (work_id, generation, generation),
            ).rowcount
            if changed != 1:
                raise ValueError("stale or already finalized claim")
            if fail:
                raise RuntimeError("crash before continuation persistence")
            if continuation:
                self.db.execute(
                    "INSERT INTO work(id, actor) SELECT ?, actor FROM work WHERE id = ?",
                    (continuation, work_id),
                )

    def cleanup(self, work_id, generation, process):
        if process.alive:
            raise ValueError("execution authority remains")
        with self.db:
            self.db.execute("BEGIN IMMEDIATE")
            changed = self.db.execute(
                "DELETE FROM reservations WHERE work_id = ? AND generation = ?",
                (work_id, generation),
            ).rowcount
            if changed != 1:
                raise ValueError("stale cleanup")
            self.db.execute(
                "UPDATE work SET state = CASE WHEN state = 'finalizing' "
                "THEN 'finished' ELSE 'interrupted' END WHERE id = ?",
                (work_id,),
            )

    def state(self, work_id):
        row = self.db.execute("SELECT state FROM work WHERE id = ?", (work_id,)).fetchone()
        return row[0] if row else None


class ContractTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "contract.sqlite"
        self.store = ContractStore(self.path)
        self.process = FakeProcess()
        self.store.accept("first")
        self.generation = self.store.claim("first")

    def tearDown(self):
        self.store.db.close()
        self.directory.cleanup()

    def test_finish_and_continuation_commit_together(self):
        with self.assertRaises(RuntimeError):
            self.store.finish("first", self.generation, "next", fail=True)
        self.assertEqual(self.store.state("first"), "running")
        self.assertIsNone(self.store.state("next"))
        self.store.finish("first", self.generation, "next")
        self.assertEqual(self.store.state("first"), "finalizing")
        self.assertEqual(self.store.state("next"), "pending")

    def test_wake_during_finalization_survives_reopen(self):
        self.store.finish("first", self.generation)
        self.store.accept("racing")
        self.store.db.close()
        self.store = ContractStore(self.path)
        self.assertIsNone(self.store.claim("racing"))
        self.process.stop()
        self.store.cleanup("first", self.generation, self.process)
        self.assertGreater(self.store.claim("racing"), self.generation)

    def test_expired_lease_does_not_release_writer(self):
        self.store.accept("next")
        self.assertIsNone(self.store.claim("next", now=1000))
        with self.assertRaises(ValueError):
            self.store.cleanup("first", self.generation, self.process)
        self.process.stop()
        self.store.cleanup("first", self.generation, self.process)
        self.assertEqual(self.store.state("first"), "interrupted")
        self.assertGreater(self.store.claim("next", now=1000), self.generation)

    def test_other_connection_cannot_claim_reserved_actor(self):
        self.store.accept("next")
        other = ContractStore(self.path)
        try:
            self.assertIsNone(other.claim("next"))
        finally:
            other.db.close()

    def test_stale_finish_and_cleanup_are_rejected(self):
        with self.assertRaises(ValueError):
            self.store.finish("first", self.generation + 1)
        self.process.stop()
        with self.assertRaises(ValueError):
            self.store.cleanup("first", self.generation + 1, self.process)
        self.assertEqual(self.store.state("first"), "running")


if __name__ == "__main__":
    unittest.main()
