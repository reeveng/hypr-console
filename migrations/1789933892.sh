# The notification store under /run, which used to be called notices.json.
#
# `console-notify` keeps what it has been told in one file so that a restart
# comes back holding the same cards rather than an empty screen. The file was
# `notices.json` and is `notifications.json`, because a crate called
# `console-notifications` had three words for one thing and this is the word
# Apple writes.
#
# What a machine is left holding is small and it is not nothing. The file is in
# `$XDG_RUNTIME_DIR/console`, which the kernel empties at boot, so the stale one
# goes by itself the next time the device is turned off. Until then it sits
# beside the live file with a day of somebody's notifications in it, and the
# `ExecStopPost` line that used to remove it now names the new file and will
# never touch it again. A file nothing reads and nothing deletes is exactly what
# this directory is for.
#
# It is swept by hand rather than by a `sweeps:` line because nothing left the
# manifest: `console-notify.service` kept its name and only its body changed.
# The gate has nothing to ask for here, which is why the reason is written out.

echo "sweeping the notification store under its old name"

for at in /run/user/*/console/notices.json
do
  console-attic "$at"
done
