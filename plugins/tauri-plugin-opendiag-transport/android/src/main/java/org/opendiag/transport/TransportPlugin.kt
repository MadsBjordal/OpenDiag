package org.opendiag.transport

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.app.PendingIntent
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.hardware.usb.UsbManager
import android.os.Build
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.hoho.android.usbserial.driver.UsbSerialPort
import com.hoho.android.usbserial.driver.UsbSerialProber
import java.io.IOException
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

@InvokeArg
class OpenArgs {
    lateinit var address: String
}

@InvokeArg
class WriteArgs {
    var id: Long = 0
    lateinit var data: String
}

@InvokeArg
class ReadArgs {
    var id: Long = 0
    var timeoutMs: Long = 100
    var max: Int = 256
}

@InvokeArg
class IdArgs {
    var id: Long = 0
}

@InvokeArg
class BreakArgs {
    var id: Long = 0
    var low: Boolean = false
}

@InvokeArg
class BaudArgs {
    var id: Long = 0
    var baud: Int = 10400
}

/** One open adapter connection: USB serial or Bluetooth RFCOMM. */
private interface Conn {
    val name: String
    fun write(bytes: ByteArray)
    fun read(max: Int, timeoutMs: Long): ByteArray
    fun close()
    /** K-line control; only USB serial cables support it. */
    fun setBreak(low: Boolean): Unit = throw IOException("This connection cannot control a K-line.")
    fun setBaud(baud: Int): Unit = throw IOException("This connection cannot change speed.")
    fun clearInput() {}
}

private class UsbConn(private val port: UsbSerialPort, override val name: String) : Conn {
    override fun write(bytes: ByteArray) = port.write(bytes, 1000)
    override fun read(max: Int, timeoutMs: Long): ByteArray {
        val buf = ByteArray(maxOf(64, max))
        val n = port.read(buf, timeoutMs.toInt().coerceAtLeast(1))
        return buf.copyOf(minOf(n, max))
    }
    override fun close() {
        try { port.close() } catch (_: IOException) {}
    }
    override fun setBreak(low: Boolean) = port.setBreak(low)
    override fun setBaud(baud: Int) = port.setParameters(baud, 8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_NONE)
    override fun clearInput() {
        try { port.purgeHwBuffers(false, true) } catch (_: Exception) {}
        val buf = ByteArray(256)
        while (port.read(buf, 1) > 0) { /* drain */ }
    }
}

private class BtConn(private val socket: BluetoothSocket, override val name: String) : Conn {
    override fun write(bytes: ByteArray) {
        socket.outputStream.write(bytes)
        socket.outputStream.flush()
    }
    override fun read(max: Int, timeoutMs: Long): ByteArray {
        val input = socket.inputStream
        val deadline = System.currentTimeMillis() + timeoutMs
        while (input.available() == 0) {
            if (!socket.isConnected) throw IOException("disconnected")
            if (System.currentTimeMillis() >= deadline) return ByteArray(0)
            Thread.sleep(5)
        }
        val buf = ByteArray(minOf(max, input.available()))
        val n = input.read(buf)
        if (n < 0) throw IOException("closed")
        return buf.copyOf(n)
    }
    override fun close() {
        try { socket.close() } catch (_: IOException) {}
    }
}

@TauriPlugin
class TransportPlugin(private val activity: Activity) : Plugin(activity) {
    private val conns = ConcurrentHashMap<Long, Conn>()
    private val nextId = AtomicLong(1)

    companion object {
        /** Standard Serial Port Profile UUID used by Bluetooth OBD adapters. */
        val SPP: UUID = UUID.fromString("00001101-0000-1000-8000-00805F9B34FB")
        const val USB_PERMISSION = "org.opendiag.transport.USB_PERMISSION"
        val BAUD_RATES = intArrayOf(38400, 115200, 9600, 230400, 500000, 1_000_000, 2_000_000)
    }

    private fun usbManager() = activity.getSystemService(Context.USB_SERVICE) as UsbManager

    private fun hasBtPermission(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.S ||
            activity.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED

    /** Runs blocking work off the main thread and reports back through the invoke. */
    private fun background(invoke: Invoke, work: () -> JSObject) {
        Thread {
            try {
                invoke.resolve(work())
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.toString())
            }
        }.start()
    }

    @SuppressLint("MissingPermission")
    @Command
    fun listDevices(invoke: Invoke) {
        val list = JSArray()
        for (driver in UsbSerialProber.getDefaultProber().findAllDrivers(usbManager())) {
            val d = driver.device
            val item = JSObject()
            item.put("address", "android-usb:${d.deviceName}")
            item.put("name", d.productName ?: "USB serial adapter")
            item.put("detail", "USB ${"%04X".format(d.vendorId)}:${"%04X".format(d.productId)} · ${driver.javaClass.simpleName.removeSuffix("SerialDriver")}")
            item.put("kind", "usb")
            list.put(item)
        }
        if (!hasBtPermission()) {
            activity.requestPermissions(arrayOf(Manifest.permission.BLUETOOTH_CONNECT), 4711)
        } else {
            val bt = (activity.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager).adapter
            bt?.bondedDevices?.forEach { dev ->
                val item = JSObject()
                item.put("address", "android-bt:${dev.address}")
                item.put("name", dev.name ?: dev.address)
                item.put("detail", "Paired Bluetooth device ${dev.address}")
                item.put("kind", "bluetooth")
                list.put(item)
            }
        }
        val ret = JSObject()
        ret.put("devices", list)
        invoke.resolve(ret)
    }

    @SuppressLint("MissingPermission")
    @Command
    fun open(invoke: Invoke) {
        val args = invoke.parseArgs(OpenArgs::class.java)
        background(invoke) {
            val conn: Conn = when {
                args.address.startsWith("android-bt:") -> openBluetooth(args.address.removePrefix("android-bt:"))
                args.address.startsWith("android-usb:") -> openUsb(args.address.removePrefix("android-usb:"))
                else -> throw IllegalArgumentException("unknown address ${args.address}")
            }
            val id = nextId.getAndIncrement()
            conns[id] = conn
            val ret = JSObject()
            ret.put("id", id)
            ret.put("name", conn.name)
            ret
        }
    }

    @SuppressLint("MissingPermission")
    private fun openBluetooth(mac: String): Conn {
        if (!hasBtPermission()) {
            activity.requestPermissions(arrayOf(Manifest.permission.BLUETOOTH_CONNECT), 4711)
            throw SecurityException("Permission for nearby devices is needed. Allow it, then connect again.")
        }
        val adapter = (activity.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager).adapter
            ?: throw IOException("This device has no Bluetooth.")
        if (!adapter.isEnabled) throw IOException("Bluetooth is off. Switch it on and try again.")
        val device = adapter.getRemoteDevice(mac)
        adapter.cancelDiscovery()
        val socket = device.createRfcommSocketToServiceRecord(SPP)
        try {
            socket.connect()
        } catch (e: IOException) {
            socket.close()
            throw IOException("Could not connect to ${device.name ?: mac}. Is the adapter plugged in and paired? (${e.message})")
        }
        return BtConn(socket, device.name ?: mac)
    }

    private fun openUsb(deviceName: String): Conn {
        val manager = usbManager()
        val driver = UsbSerialProber.getDefaultProber().findAllDrivers(manager).firstOrNull { it.device.deviceName == deviceName }
            ?: throw IOException("The USB adapter is no longer connected.")
        if (!manager.hasPermission(driver.device)) {
            val intent = Intent(USB_PERMISSION).setPackage(activity.packageName)
            val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
            manager.requestPermission(driver.device, PendingIntent.getBroadcast(activity, 0, intent, flags))
            throw SecurityException("USB permission requested. Allow it, then press Connect again.")
        }
        val connection = manager.openDevice(driver.device) ?: throw IOException("Android could not open the USB device.")
        val port = driver.ports[0]
        port.open(connection)
        // Find the speed the adapter uses by asking for its identification (ATI, an adapter-only command).
        for (baud in BAUD_RATES) {
            port.setParameters(baud, 8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_NONE)
            try { port.purgeHwBuffers(true, true) } catch (_: Exception) {}
            port.write("\rATI\r".toByteArray(), 500)
            val deadline = System.currentTimeMillis() + 1200
            val got = StringBuilder()
            val buf = ByteArray(128)
            while (System.currentTimeMillis() < deadline) {
                val n = port.read(buf, 100)
                if (n > 0) got.append(String(buf, 0, n, Charsets.ISO_8859_1))
                if (looksLikeElm(got.toString())) {
                    Thread.sleep(100)
                    while (port.read(buf, 50) > 0) { /* drain */ }
                    return UsbConn(port, "${driver.device.productName ?: "USB adapter"} @ $baud")
                }
            }
        }
        port.close()
        throw IOException("No ELM327-compatible answer from the USB adapter at any common speed.")
    }

    /**
     * Whether an answer to ATI comes from an ELM327-compatible adapter: a known
     * family name, or clean text ending in the ELM327 prompt (some compatible
     * adapters, e.g. Vgate vLinker, use their own name). Same rule as
     * `looks_like_elm` in crates/opendiag-transport/src/serial.rs.
     */
    private fun looksLikeElm(answer: String): Boolean {
        val up = answer.uppercase()
        if (listOf("ELM", "STN", "VLINKER", "VGATE", "OBDLINK").any { up.contains(it) }) return true
        val end = answer.lastIndexOf('>')
        if (end < 0) return false
        val body = answer.substring(0, end)
        val clean = body.all { it == ' ' || it == '\r' || it == '\n' || it == '\u0000' || (it in '!'..'~') }
        return clean && body.count { it.isLetterOrDigit() && it.code < 128 } >= 3
    }

    /** Opens a USB serial K-line cable at 10400 baud without probing for an ELM327. */
    @Command
    fun openKline(invoke: Invoke) {
        val args = invoke.parseArgs(OpenArgs::class.java)
        background(invoke) {
            if (!args.address.startsWith("android-usb:")) throw IOException("K-line needs a USB cable; Bluetooth adapters cannot drive a K-line.")
            val conn = openUsbRaw(args.address.removePrefix("android-usb:"), 10400)
            val id = nextId.getAndIncrement()
            conns[id] = conn
            val ret = JSObject()
            ret.put("id", id)
            ret.put("name", conn.name)
            ret
        }
    }

    private fun openUsbRaw(deviceName: String, baud: Int): Conn {
        val manager = usbManager()
        val driver = UsbSerialProber.getDefaultProber().findAllDrivers(manager).firstOrNull { it.device.deviceName == deviceName }
            ?: throw IOException("The USB cable is no longer connected.")
        if (!manager.hasPermission(driver.device)) {
            val intent = Intent(USB_PERMISSION).setPackage(activity.packageName)
            val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
            manager.requestPermission(driver.device, PendingIntent.getBroadcast(activity, 0, intent, flags))
            throw SecurityException("USB permission requested. Allow it, then press Connect again.")
        }
        val connection = manager.openDevice(driver.device) ?: throw IOException("Android could not open the USB device.")
        val port = driver.ports[0]
        port.open(connection)
        port.setParameters(baud, 8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_NONE)
        return UsbConn(port, "${driver.device.productName ?: "USB K-line cable"} (K-line)")
    }

    @Command
    fun setBreak(invoke: Invoke) {
        val args = invoke.parseArgs(BreakArgs::class.java)
        background(invoke) {
            (conns[args.id] ?: throw IOException("closed")).setBreak(args.low)
            JSObject()
        }
    }

    @Command
    fun setBaud(invoke: Invoke) {
        val args = invoke.parseArgs(BaudArgs::class.java)
        background(invoke) {
            (conns[args.id] ?: throw IOException("closed")).setBaud(args.baud)
            JSObject()
        }
    }

    @Command
    fun clearInput(invoke: Invoke) {
        val args = invoke.parseArgs(IdArgs::class.java)
        background(invoke) {
            (conns[args.id] ?: throw IOException("closed")).clearInput()
            JSObject()
        }
    }

    @Command
    fun write(invoke: Invoke) {
        val args = invoke.parseArgs(WriteArgs::class.java)
        background(invoke) {
            val conn = conns[args.id] ?: throw IOException("closed")
            conn.write(Base64.decode(args.data, Base64.NO_WRAP))
            JSObject()
        }
    }

    @Command
    fun read(invoke: Invoke) {
        val args = invoke.parseArgs(ReadArgs::class.java)
        background(invoke) {
            val conn = conns[args.id] ?: throw IOException("closed")
            val bytes = conn.read(args.max, args.timeoutMs)
            val ret = JSObject()
            ret.put("data", Base64.encodeToString(bytes, Base64.NO_WRAP))
            ret
        }
    }

    @Command
    fun close(invoke: Invoke) {
        val args = invoke.parseArgs(IdArgs::class.java)
        conns.remove(args.id)?.close()
        invoke.resolve(JSObject())
    }
}
