"""Sample memory, paging and CPU counters of a Linux machine every 2 seconds.

    python probe_sampler.py PID > samples.tsv      # until PID exits
    python probe_sampler.py --summary samples.tsv START END   # one time window

Shows whether a process whose model is memory-mapped thrashes: major page
faults (pages re-read from disk), disk reads, swapping, CPU time spent
waiting for the disk, and the kernel's pressure-stall figures, alongside
how much of the process's memory is file-backed and how much is its own.
Used by the temporary model probe workflow.
"""

import sys
import time

COLUMNS = ["t", "majflt", "read_mib", "swapin_mib", "swapout_mib", "cpu_busy", "cpu_iowait", "cpu_total",
           "mem_some", "mem_full", "io_some", "io_full", "avail_mib", "swap_used_mib", "rss_anon_mib", "rss_file_mib"]


def vmstat():
    v = {}
    for line in open("/proc/vmstat"):
        k, n = line.split()
        v[k] = int(n)
    return v


def cpu():
    f = [int(x) for x in open("/proc/stat").readline().split()[1:]]
    user, nice, system, idle, iowait, irq, softirq, steal = f[:8]
    return user + nice + system + irq + softirq + steal, iowait, sum(f[:8])


def pressure(kind):
    out = {}
    try:
        for line in open(f"/proc/pressure/{kind}"):
            name, *fields = line.split()
            out[name] = float(dict(x.split("=") for x in fields)["avg10"])
    except OSError:
        pass
    return out.get("some", -1.0), out.get("full", -1.0)


def meminfo():
    m = {}
    for line in open("/proc/meminfo"):
        k, v = line.split(":")
        m[k] = int(v.split()[0]) // 1024
    return m["MemAvailable"], m["SwapTotal"] - m["SwapFree"]


def proc_rss(pid):
    anon = file = 0
    try:
        for line in open(f"/proc/{pid}/status"):
            if line.startswith("RssAnon:"):
                anon = int(line.split()[1]) // 1024
            elif line.startswith("RssFile:"):
                file = int(line.split()[1]) // 1024
    except OSError:
        return None
    return anon, file


def sample(pid):
    print("\t".join(COLUMNS), flush=True)
    while True:
        rss = proc_rss(pid)
        if rss is None:
            return
        v, (busy, iowait, total) = vmstat(), cpu()
        avail, swap = meminfo()
        row = [round(time.time(), 1), v["pgmajfault"], v["pgpgin"] // 1024, v["pswpin"] * 4 // 1024, v["pswpout"] * 4 // 1024,
               busy, iowait, total, *pressure("memory"), *pressure("io"), avail, swap, *rss]
        print("\t".join(str(x) for x in row), flush=True)
        time.sleep(2)


def summary(path, start, end):
    rows = [dict(zip(COLUMNS, map(float, line.split("\t")))) for line in list(open(path))[1:]]
    w = [r for r in rows if start <= r["t"] <= end]
    if len(w) < 2:
        print("too few samples")
        return
    a, b = w[0], w[-1]
    secs = b["t"] - a["t"]
    d = lambda k: b[k] - a[k]
    total = d("cpu_total") or 1
    print(f"  over {secs:.0f}s: {d('majflt'):.0f} major page faults ({d('majflt') / secs:.1f}/s), "
          f"{d('read_mib'):.0f} MiB read from disk ({d('read_mib') / secs:.1f} MiB/s), "
          f"swap in {d('swapin_mib'):.0f} MiB / out {d('swapout_mib'):.0f} MiB")
    print(f"  CPU busy {100 * d('cpu_busy') / total:.0f}%, waiting for disk {100 * d('cpu_iowait') / total:.1f}%; "
          f"pressure (max avg10) memory some {max(r['mem_some'] for r in w):.1f}% full {max(r['mem_full'] for r in w):.1f}%, "
          f"io some {max(r['io_some'] for r in w):.1f}% full {max(r['io_full'] for r in w):.1f}%")
    print(f"  lowest available {min(r['avail_mib'] for r in w):.0f} MiB, most swap used {max(r['swap_used_mib'] for r in w):.0f} MiB, "
          f"server memory: own {max(r['rss_anon_mib'] for r in w):.0f} MiB, file-backed {min(r['rss_file_mib'] for r in w):.0f}-{max(r['rss_file_mib'] for r in w):.0f} MiB")


if __name__ == "__main__":
    if sys.argv[1] == "--summary":
        summary(sys.argv[2], float(sys.argv[3]), float(sys.argv[4]))
    else:
        sample(int(sys.argv[1]))
