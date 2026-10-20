import { useEffect, useState } from "react";
import { call, inApp } from "../api";
import {
  ErrorBox,
  SimBanner,
  Spinner,
  StatusChip,
  useAction,
} from "../components";
import { t } from "../i18n";
import { useApp } from "../state";
import { protocolName } from "../units";
import type { Device, Summary } from "../types";

type Kind = "usb" | "bluetooth" | "wifi" | "kline" | "sim";

const KINDS: { kind: Kind; icon: string; title: string; text: string }[] = [
  {
    kind: "usb",
    icon: "🔌",
    title: "USB or USB-C cable",
    text: "Adapter plugged into this device",
  },
  {
    kind: "bluetooth",
    icon: "📶",
    title: "Bluetooth",
    text: "Wireless adapter, paired first",
  },
  {
    kind: "wifi",
    icon: "🛜",
    title: "Wi-Fi or Ethernet",
    text: "Adapter with its own network",
  },
  {
    kind: "kline",
    icon: "〰",
    title: "K-line cable (KKL / FTDI)",
    text: "Wired cable for older cars and motorcycles",
  },
  {
    kind: "sim",
    icon: "🧪",
    title: "Simulator",
    text: "Try OpenDiag without a vehicle",
  },
];

const isKlineSim = (d: Device) => d.address.startsWith("kline-sim:");

const matches = (d: Device, k: Kind) =>
  k === "kline"
    ? (isKlineSim(d) && !d.address.endsWith(":honda")) ||
      d.kind === "usb_serial" ||
      d.kind === "android_usb"
    : k === "sim"
      ? d.simulated && !isKlineSim(d)
      : k === "wifi"
        ? d.kind === "wifi" || d.kind === "ethernet"
        : k === "bluetooth"
          ? d.kind === "bluetooth_classic" || d.kind === "bluetooth_le"
          : d.kind === "usb_serial" || d.kind === "android_usb";

function Help({ kind, platform }: { kind: Kind; platform: string }) {
  const android = platform === "android";
  const tips: Record<Kind, string[]> = {
    usb: android
      ? [
          "Plug the adapter in with a USB-C OTG cable. Android asks for permission the first time: allow it.",
          "Supported USB chips: FTDI, CH340/CH341, CP210x, PL2303 and CDC-ACM.",
        ]
      : [
          "Plug the adapter in. On Windows it appears as a COM port; if not, install the driver for its USB chip (FTDI, CH340 or CP210x) from the chip maker.",
          'On Linux, add yourself to the "dialout" group if the port is refused.',
          "USB-C is just the plug: what matters is the adapter inside.",
        ],
    bluetooth: android
      ? [
          "Pair the adapter in Android's Bluetooth settings first (PIN is often 1234 or 0000).",
          'Allow the "Nearby devices" permission when asked.',
          "Bluetooth Low Energy adapters are not supported yet.",
        ]
      : [
          "Pair the adapter in Windows Bluetooth settings first. Windows then creates an outgoing COM port for it: choose that port below.",
          "Bluetooth Low Energy adapters are not supported yet.",
        ],
    wifi: [
      "Connect this device to the adapter's Wi-Fi network first (mobile data may need to be off).",
      "Many adapters use 192.168.0.10, port 35000. Check your adapter's manual.",
    ],
    kline: [
      'A K-line cable is a plain USB cable with an FTDI-type chip, often sold as "KKL" or "VAG-COM 409.1". OpenDiag drives the K-line itself, so a normal ELM327 adapter is not used here.',
      "It reaches generic OBD-II on K-line vehicles (ISO 9141-2 and KWP2000), typically cars from about 2000 to 2008 and some motorcycles with a suitable adapter cable.",
      android
        ? "On Android plug it in with a USB-C OTG cable; Bluetooth cannot drive a K-line."
        : "On Windows, set the FTDI latency timer to 1 ms (Device Manager → Ports → the cable → Port Settings → Advanced) for reliable timing.",
      "Ignition on, engine off or idling. The slow (5-baud) wake-up takes about 3 seconds.",
      "Not tested on real vehicles yet: please report what happens.",
    ],
    sim: [
      "Simulated vehicles are always labelled SIMULATED. Use the simulator controls to switch the engine off or create faults.",
    ],
  };
  return (
    <ul className="tight muted small">
      {tips[kind].map((x) => (
        <li key={x}>{t(x)}</li>
      ))}
    </ul>
  );
}

export default function Connect() {
  const { go, settings, setSummary, summary, setReport } = useApp();
  const [kind, setKind] = useState<Kind | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [address, setAddress] = useState("192.168.0.10:35000");
  const [init, setInit] = useState<"auto" | "fast" | "5baud">("auto");
  const klineAddress = (d: Device) =>
    isKlineSim(d)
      ? `${d.address}#${init}`
      : `kline:${d.address.replace(/^serial:/, "")}#${init}`;
  const list = useAction(async () =>
    setDevices(await call<Device[]>("devices")),
  );
  const connect = useAction(async (addr: string) => {
    const s = await call<Summary>("connect", { address: addr });
    setSummary(s);
    setReport(null);
    return s;
  });
  const retry = useAction(async () =>
    setSummary(await call<Summary>("connect_vehicle")),
  );
  const disconnect = useAction(async () => {
    await call("disconnect");
    setSummary(null);
    setReport(null);
  });
  useEffect(() => {
    if (kind) void list.run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind]);
  const platform = settings?.platform ?? "linux";
  const shown = kind
    ? devices.filter((d) => matches(d, kind) && d.id !== "tcp")
    : [];

  if (summary) {
    const usable = summary.compatibility.functions.filter(
      (f) =>
        f.risk_level <= 1 &&
        f.status !== "planned" &&
        f.status !== "unsupported" &&
        !f.id.startsWith("connect_"),
    );
    return (
      <>
        <h1>{t("Connection")}</h1>
        <SimBanner show={summary.simulated} />
        <div className="card">
          <h2 style={{ marginTop: 0 }}>{t("Step 3 · Adapter")}</h2>
          <p>
            <b>{summary.adapter.identity}</b>{" "}
            {summary.adapter.description && (
              <span className="muted">· {summary.adapter.description}</span>
            )}
          </p>
          <p className="muted">{summary.link.name}</p>
          {summary.adapter.clone_suspected && (
            <p className="banner warn">
              {t(
                "This adapter looks like a clone. It may not support every feature it claims.",
              )}
            </p>
          )}
          {summary.adapter.notes.map((n) => (
            <p key={n} className="muted small">
              {n}
            </p>
          ))}
          <div className="row small">
            <span
              className={`chip ${summary.adapter.headers === "yes" ? "ok" : "warn"}`}
            >
              {t("Shows which module answers")}: {summary.adapter.headers}
            </span>
            <span
              className={`chip ${summary.adapter.voltage_reading === "yes" ? "ok" : ""}`}
            >
              {t("Battery voltage")}: {summary.adapter.voltage_reading}
            </span>
            <span className="chip">
              {t("Long answers")}: {summary.adapter.multi_frame}
            </span>
          </div>
        </div>
        <div className="card">
          <h2 style={{ marginTop: 0 }}>{t("Step 4 · Vehicle")}</h2>
          {summary.vehicle_connected ? (
            <>
              <p className="banner ok">
                {t("Connected using {p}.", {
                  p: protocolName(summary.protocol),
                })}{" "}
                {t("{n} module(s) answered.", { n: summary.ecus.length })}
              </p>
              <ul className="tight">
                {summary.ecus.map((e) => (
                  <li key={e.address}>
                    {e.label} <span className="muted mono">{e.address}</span> ·{" "}
                    {t("{n} data values", { n: e.supported_pids })}
                  </li>
                ))}
              </ul>
              <p className="muted small">
                {t(
                  "The vehicle is identified during the scan (VIN). You can name it and add details in My Garage.",
                )}
              </p>
            </>
          ) : (
            <>
              <p className="banner warn">
                {summary.vehicle_error ?? t("The vehicle did not answer.")}
              </p>
              <p>
                {t(
                  "Switch the ignition on (engine can stay off), check the adapter is fully plugged into the diagnostic socket, then try again.",
                )}
              </p>
              <button
                className="btn primary"
                onClick={() => retry.run()}
                disabled={retry.busy}
              >
                {retry.busy ? <Spinner /> : null} {t("Try the vehicle again")}
              </button>
              <ErrorBox error={retry.error} />
              <p className="small" style={{ marginTop: 12 }}>
                {t(
                  "Truck, bus or machine? These often use J1939 instead of OBD-II.",
                )}{" "}
                <button className="btn small" onClick={() => go("truck")}>
                  🚛 {t("Try J1939")}
                </button>
              </p>
            </>
          )}
        </div>
        <div className="card">
          <h2 style={{ marginTop: 0 }}>
            {t("Step 5 · What this adapter can do here")}
          </h2>
          <p className="muted">{summary.compatibility.summary}</p>
          <table>
            <tbody>
              {usable.map((f) => (
                <tr key={f.id}>
                  <td>{f.name}</td>
                  <td>
                    <StatusChip status={f.status} label={f.status_label} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="row" style={{ marginTop: 16 }}>
          <button
            className="btn primary"
            disabled={!summary.vehicle_connected}
            onClick={() => go("scan", "start")}
          >
            {t("Step 6 · Start the scan")}
          </button>
          <button className="btn" onClick={() => disconnect.run()}>
            {t("Disconnect")}
          </button>
        </div>
      </>
    );
  }

  return (
    <>
      <h1>{t("Connect to Vehicle")}</h1>
      <p className="muted">
        {t(
          "Plug the adapter into the vehicle's diagnostic socket (usually under the dashboard; on motorcycles often under the seat, with an adapter cable) and switch the ignition on.",
        )}
      </p>
      <div className="banner info">
        {t(
          "Not every vehicle has the standard 16-pin socket. Motorcycles, older cars, trucks and boats often use other connectors, and some only flash codes.",
        )}{" "}
        <button className="btn small" onClick={() => go("connectors")}>
          🔍 {t("Find my connector")}
        </button>
      </div>
      <h2>{t("Step 1 · How is your adapter connected?")}</h2>
      <div className="grid tiles">
        {KINDS.map((k) => (
          <button
            key={k.kind}
            className={`tile ${kind === k.kind ? "primary" : ""}`}
            onClick={() => setKind(k.kind)}
          >
            <span className="ico" aria-hidden="true">
              {k.icon}
            </span>
            <b>{t(k.title)}</b>
            <span className="muted small">{t(k.text)}</span>
          </button>
        ))}
      </div>
      {kind && (
        <div className="card" style={{ marginTop: 16 }}>
          <div className="row between">
            <h2 style={{ margin: 0 }}>{t("Step 2 · Choose the adapter")}</h2>
            {kind !== "wifi" && (
              <button
                className="btn small"
                onClick={() => list.run()}
                disabled={list.busy}
              >
                {list.busy ? <Spinner /> : "↻"} {t("Search again")}
              </button>
            )}
          </div>
          <Help kind={kind} platform={platform} />
          {!inApp() && kind !== "sim" && (
            <p className="muted small">
              {t(
                "Running in a browser: devices are those the development bridge can see on this computer.",
              )}
            </p>
          )}
          <ErrorBox error={list.error} />
          {kind === "kline" && (
            <label className="field" style={{ maxWidth: 360, marginTop: 8 }}>
              {t("Wake-up")}
              <select
                value={init}
                onChange={(e) => setInit(e.target.value as typeof init)}
              >
                <option value="auto">
                  {t("Automatic (fast init, then 5-baud)")}
                </option>
                <option value="fast">
                  {t("Fast init (ISO 14230-4 KWP2000)")}
                </option>
                <option value="5baud">
                  {t("5-baud (ISO 9141-2 or KWP2000)")}
                </option>
              </select>
            </label>
          )}
          {kind === "wifi" ? (
            <div className="row" style={{ marginTop: 10 }}>
              <label className="field" style={{ flex: 1, minWidth: 220 }}>
                {t("Address and port")}
                <input
                  type="text"
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                  placeholder="192.168.0.10:35000"
                />
              </label>
              <button
                className="btn primary"
                style={{ alignSelf: "flex-end" }}
                disabled={connect.busy}
                onClick={() => connect.run(`tcp:${address.trim()}`)}
              >
                {connect.busy ? <Spinner /> : null} {t("Connect")}
              </button>
            </div>
          ) : shown.length === 0 && !list.busy ? (
            <p className="empty">
              {kind === "sim"
                ? t("No simulator profiles.")
                : t(
                    "No adapters found. Check the tips above, then search again.",
                  )}
            </p>
          ) : (
            <div className="stack" style={{ marginTop: 10 }}>
              {shown.map((d) => (
                <div
                  key={d.id}
                  className="row between card"
                  style={{ padding: 12 }}
                >
                  <div>
                    <b>{d.name}</b>{" "}
                    {d.simulated && (
                      <span className="chip sim">{t("SIMULATED")}</span>
                    )}
                    <div className="muted small">{d.detail}</div>
                  </div>
                  <button
                    className="btn primary"
                    disabled={connect.busy}
                    onClick={() =>
                      connect.run(
                        kind === "kline" ? klineAddress(d) : d.address,
                      )
                    }
                  >
                    {connect.busy ? <Spinner /> : null} {t("Connect")}
                  </button>
                </div>
              ))}
            </div>
          )}
          <ErrorBox error={connect.error} />
        </div>
      )}
    </>
  );
}
