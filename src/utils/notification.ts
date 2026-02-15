import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

export async function notify(title: string, body?: string) {
  console.log(`[Notification] Attempting to send: ${title} - ${body}`);
  let permissionGranted = await isPermissionGranted();
  console.log(
    `[Notification] Permission granted initially: ${permissionGranted}`,
  );

  if (!permissionGranted) {
    const permission = await requestPermission();
    console.log(`[Notification] Permission request result: ${permission}`);
    permissionGranted = permission === "granted";
  }

  if (permissionGranted) {
    console.log("[Notification] Sending notification...");
    try {
      sendNotification({ title, body });
      console.log("[Notification] Notification sent to system");
    } catch (e) {
      console.error("[Notification] Failed to send notification:", e);
    }
  } else {
    console.warn("[Notification] Notification permission denied");
  }
}
