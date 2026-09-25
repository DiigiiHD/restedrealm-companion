"""Small Windows interface for the local queue and future paired uploads."""

import json
import threading
import time
import tkinter as tk
from tkinter import messagebox, ttk
import webbrowser

from restedrealm_companion import DEFAULT_GAME, DEFAULT_STATE, compact_one, connect, forget_queue, saved_files, scan_one
import uploader
import windows_credentials


class CompanionWindow:
    def __init__(self, root):
        self.root = root
        self.root.title("RestedRealm Collector")
        self.root.geometry("760x480")
        self.root.minsize(620, 380)
        self.database = connect(DEFAULT_STATE)
        self.watching = True
        self.watch_generation = 0
        self.last_error = None
        self.uploading = False
        self.next_retry = 0.0
        self.retry_delay = 30
        self.upload_opt_in = tk.BooleanVar(value=self.database.execute(
            "SELECT value FROM settings WHERE key='upload_opt_in'").fetchone() == ("1",))

        header = ttk.Frame(root, padding=14)
        header.pack(fill="x")
        ttk.Label(header, text="RestedRealm Collector", font=("Segoe UI", 16, "bold")).pack(anchor="w")
        ttk.Label(header, text="Forever observations stay in your local queue until you choose Upload now or turn on automatic upload.").pack(anchor="w", pady=(3, 0))

        buttons = ttk.Frame(root, padding=(14, 0, 14, 8))
        buttons.pack(fill="x")
        ttk.Button(buttons, text="Scan saved data", command=self.scan).pack(side="left")
        self.watch_button = ttk.Button(buttons, text="Pause watching", command=self.toggle_watch)
        self.watch_button.pack(side="left", padx=8)
        ttk.Button(buttons, text="Free addon space", command=self.rollover).pack(side="left")
        ttk.Button(buttons, text="Forget local queue", command=self.forget).pack(side="right")

        self.status_text = tk.StringVar(value="Checking local queue...")
        ttk.Label(root, textvariable=self.status_text, padding=(14, 0, 14, 8)).pack(anchor="w")

        if uploader.WEBSITE_READY:
            connection = ttk.Frame(root, padding=(14, 0, 14, 12))
            connection.pack(fill="x")
            self.connection_text = tk.StringVar()
            ttk.Label(connection, textvariable=self.connection_text).pack(side="left", padx=(0, 12))
            self.refresh_connection()
            ttk.Button(connection, text="Open RestedRealm account page", command=lambda:
                       webbrowser.open("https://restedrealm.com/account/collector")).pack(side="left")
            self.pair_code = tk.StringVar()
            ttk.Entry(connection, textvariable=self.pair_code, width=20).pack(side="left", padx=8)
            ttk.Button(connection, text="Connect this PC", command=self.pair).pack(side="left")
            if uploader.UPLOAD_READY:
                ttk.Button(connection, text="Upload now", command=self.start_upload).pack(side="left", padx=8)
                ttk.Checkbutton(root, text="Upload automatically after a completed save", variable=self.upload_opt_in,
                                command=self.save_upload_choice, padding=(14, 0, 14, 10)).pack(anchor="w")

        columns = ("id", "seq", "kind", "date")
        self.table = ttk.Treeview(root, columns=columns, show="headings", selectmode="browse")
        for key, label, width in (("id", "ID", 105), ("seq", "Sequence", 75),
                                  ("kind", "Observation", 240), ("date", "Observed (UTC)", 170)):
            self.table.heading(key, text=label)
            self.table.column(key, width=width, minwidth=70)
        self.table.pack(fill="both", expand=True, padx=14, pady=(0, 5))
        self.table.bind("<Double-1>", self.show_record)
        ttk.Label(root, text="Double-click an observation to inspect its full locally saved data.",
                  padding=(14, 0, 14, 12)).pack(anchor="w")
        self.refresh()
        self.root.after(100, lambda: self.scan(quiet=True))
        self.root.after(30000, self.poll, self.watch_generation)
        self.root.protocol("WM_DELETE_WINDOW", self.close)

    def refresh(self):
        count = self.database.execute("SELECT COUNT(*) FROM observations").fetchone()[0]
        pending = self.database.execute("SELECT COUNT(*) FROM observations WHERE uploaded_digest IS NULL OR uploaded_digest<>digest").fetchone()[0]
        drops = self.database.execute("SELECT COALESCE(SUM(dropped), 0) FROM imports").fetchone()[0]
        self.status_text.set(f"{count} observations queued locally  |  {pending} awaiting upload  |  {drops} addon drops  |  "
                             f"Watching: {'on' if self.watching else 'off'}" +
                             (f"  |  Last check: {self.last_error}" if self.last_error else ""))
        self.table.delete(*self.table.get_children())
        rows = self.database.execute("SELECT source_id, seq, digest, payload FROM observations "
                                     "ORDER BY queued_at DESC, seq DESC LIMIT 300").fetchall()
        for source_id, seq, digest, payload in rows:
            record = json.loads(payload)
            observed = record.get("observedAt")
            from datetime import datetime, timezone
            when = datetime.fromtimestamp(observed, timezone.utc).strftime("%Y-%m-%d %H:%M") if type(observed) is int and 0 < observed < 4102444800 else "unknown"
            self.table.insert("", "end", iid=source_id + ":" + str(seq),
                              values=(digest[:12], seq, record.get("kind", "?"), when))

    def refresh_connection(self):
        try:
            paired = bool(windows_credentials.load())
            self.connection_text.set("Connected to RestedRealm" if paired else "This PC is not connected")
        except (OSError, ValueError):
            self.connection_text.set("Connection status unavailable")

    def scan(self, quiet=False):
        paths = saved_files(DEFAULT_GAME)
        if not paths:
            if not quiet:
                messagebox.showinfo("No save found", "No RestedRealm Collector save was found in the Forever installation. Log out or use /reload in WoW first.")
            return
        new = updated = 0
        errors = []
        for path in paths:
            try:
                result = scan_one(self.database, path)
                new += result["new"]
                updated += result["updated"]
            except (OSError, UnicodeError, ValueError) as exc:
                errors.append(type(exc).__name__ + ": " + str(exc))
        self.last_error = "save not ready" if errors else None
        self.refresh()
        if uploader.UPLOAD_READY and self.upload_opt_in.get() and not errors and time.monotonic() >= self.next_retry:
            self.start_upload(quiet=True)
        if errors and not quiet:
            messagebox.showwarning("Save not imported", "The game save may still be writing. The existing queue is safe.\n\n" + "\n".join(errors))
        elif not quiet:
            messagebox.showinfo("Local import complete", f"{new} new observations queued; {updated} corrected observations updated.")

    def toggle_watch(self):
        self.watching = not self.watching
        self.watch_generation += 1
        self.watch_button.config(text="Pause watching" if self.watching else "Watch saves")
        self.refresh()
        if self.watching:
            self.scan(quiet=True)
            self.root.after(30000, self.poll, self.watch_generation)

    def poll(self, generation):
        if self.watching and generation == self.watch_generation:
            self.scan(quiet=True)
            self.root.after(30000, self.poll, generation)

    def show_record(self, _event):
        selected = self.table.selection()
        if not selected:
            return
        source_id, seq = selected[0].split(":", 1)
        row = self.database.execute("SELECT payload FROM observations WHERE source_id=? AND seq=?",
                                    (source_id, int(seq))).fetchone()
        if not row:
            return
        popup = tk.Toplevel(self.root)
        popup.title("Locally saved observation")
        popup.geometry("700x550")
        view = tk.Text(popup, wrap="word")
        view.pack(fill="both", expand=True)
        view.insert("1.0", json.dumps(json.loads(row[0]), indent=2, ensure_ascii=False))
        view.config(state="disabled")

    def rollover(self):
        if not messagebox.askyesno("Free addon space", "After WoW is fully closed, remove observations already copied into this companion's queue from the addon save? A private backup will be kept."):
            return
        paths = saved_files(DEFAULT_GAME)
        if not paths:
            messagebox.showinfo("No save found", "No RestedRealm Collector save was found.")
            return
        total = 0
        try:
            for path in paths:
                total += compact_one(self.database, path, DEFAULT_STATE)
        except (OSError, UnicodeError, ValueError, RuntimeError) as exc:
            messagebox.showwarning("Space not freed", str(exc) + "\n\nThe companion queue was not deleted.")
            return
        self.refresh()
        messagebox.showinfo("Addon space freed", f"{total} safely queued observations removed from the closed game's save. Your companion queue and a backup are retained.")

    def forget(self):
        if self.uploading:
            messagebox.showinfo("Upload in progress", "Wait for the current upload to finish before clearing the local queue.")
            return
        if not messagebox.askyesno("Forget local queue", "Delete the companion's queued observations and turn off automatic upload? The game's SavedVariables file and any rollover backup remain on this PC."):
            return
        self.watching = False
        self.watch_generation += 1
        self.watch_button.config(text="Watch saves")
        self.upload_opt_in.set(False)
        forget_queue(self.database)
        self.refresh()

    def save_upload_choice(self):
        with self.database:
            self.database.execute(
                "INSERT INTO settings(key,value) VALUES('upload_opt_in',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                ("1" if self.upload_opt_in.get() else "0",),
            )
        if self.upload_opt_in.get():
            self.start_upload(quiet=True)

    def pair(self):
        code = self.pair_code.get().strip()
        if not code:
            messagebox.showinfo("Connection code", "Generate a code on your RestedRealm account page and enter it here.")
            return
        try:
            uploader.redeem_code(code)
            self.pair_code.set("")
            self.refresh_connection()
            messagebox.showinfo("Connected", "This PC is paired with your RestedRealm account. Gameplay uploads remain locked until the pilot check is complete.")
            if self.upload_opt_in.get():
                self.start_upload(quiet=True)
        except (OSError, ValueError, RuntimeError) as exc:
            messagebox.showwarning("Could not connect", str(exc))

    def start_upload(self, quiet=False):
        if not uploader.UPLOAD_READY or self.uploading:
            return
        try:
            if not windows_credentials.load():
                if not quiet:
                    messagebox.showinfo("Connect first", "Connect this PC through your RestedRealm account page first.")
                return
        except (OSError, ValueError) as exc:
            if not quiet:
                messagebox.showwarning("Credential unavailable", str(exc))
            return
        self.uploading = True
        def work():
            database = connect(DEFAULT_STATE)
            try:
                count = 0
                for _ in range(20):
                    sent = uploader.upload_once(database)
                    count += sent
                    if sent == 0:
                        break
                result = (count, None)
            except (OSError, ValueError, RuntimeError) as exc:
                result = (0, str(exc))
            finally:
                database.close()
            try:
                self.root.after(0, self.upload_finished, *result, quiet)
            except RuntimeError:
                pass
        threading.Thread(target=work, daemon=True).start()

    def upload_finished(self, count, error, quiet):
        self.uploading = False
        if error:
            self.last_error = "upload paused: " + error
            self.next_retry = time.monotonic() + self.retry_delay
            self.retry_delay = min(self.retry_delay * 2, 3600)
            if not quiet:
                messagebox.showwarning("Upload did not finish", error + "\nThe local queue is safe and can retry.")
        else:
            self.last_error = None
            self.retry_delay = 30
            self.next_retry = 0
            if not quiet:
                messagebox.showinfo("Upload complete", f"{count} observations acknowledged by RestedRealm.")
        self.refresh()

    def close(self):
        self.database.close()
        self.root.destroy()


if __name__ == "__main__":
    window = tk.Tk()
    CompanionWindow(window)
    window.mainloop()
