package rs.liber.app

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class OpenFileArgs {
  lateinit var path: String
  var mime: String? = null
  var name: String? = null
}

@TauriPlugin
class OpenFilePlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun open(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(OpenFileArgs::class.java)
      val file = existingFile(args.path)
      val intent = Intent(Intent.ACTION_VIEW)
        .setDataAndType(contentUri(file), args.mime ?: "application/octet-stream")
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      activity.startActivity(intent)
      invoke.resolve()
    } catch (ex: Exception) {
      invoke.reject(ex.message ?: "could not open file")
    }
  }

  @Command
  fun share(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(OpenFileArgs::class.java)
      val file = existingFile(args.path)
      val uri = contentUri(file)
      val intent = Intent(Intent.ACTION_SEND).apply {
        type = args.mime ?: "*/*"
        putExtra(Intent.EXTRA_STREAM, uri)
        clipData = ClipData.newRawUri(null, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      }
      activity.startActivity(Intent.createChooser(intent, null))
      invoke.resolve()
    } catch (ex: Exception) {
      invoke.reject(ex.message ?: "could not share file")
    }
  }

  @Command
  fun save(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(OpenFileArgs::class.java)
      val file = existingFile(args.path)
      val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
        .addCategory(Intent.CATEGORY_OPENABLE)
        .setType(args.mime ?: "application/octet-stream")
        .putExtra(Intent.EXTRA_TITLE, args.name ?: file.name)
      startActivityForResult(invoke, intent, "saveResult")
    } catch (ex: Exception) {
      invoke.reject(ex.message ?: "could not save file")
    }
  }

  @ActivityCallback
  fun saveResult(invoke: Invoke, result: ActivityResult) {
    try {
      if (result.resultCode != Activity.RESULT_OK) {
        invoke.reject("save cancelled")
        return
      }
      val args = invoke.parseArgs(OpenFileArgs::class.java)
      val source = existingFile(args.path)
      val uri = result.data?.data ?: throw IllegalStateException("no destination selected")
      Thread {
        try {
          activity.contentResolver.openOutputStream(uri)?.use { out ->
            source.inputStream().use { input -> input.copyTo(out) }
          } ?: throw IllegalStateException("could not open destination")
          activity.runOnUiThread { invoke.resolve() }
        } catch (ex: Exception) {
          activity.runOnUiThread { invoke.reject(ex.message ?: "save failed") }
        }
      }.start()
    } catch (ex: Exception) {
      invoke.reject(ex.message ?: "save failed")
    }
  }

  private fun existingFile(path: String): File {
    val file = File(path)
    if (!file.exists()) throw IllegalArgumentException("file not found")
    return file
  }

  private fun contentUri(file: File): Uri =
    FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
}
