const active = ref(false);
const message = ref("");
const targetName = ref<string | null>(null);

export function beginProfileSwitch(name: string) {
  active.value = true;
  targetName.value = name;
  message.value = `Switching to ${name}…`;
}

export function setProfileSwitchMessage(nextMessage: string) {
  message.value = nextMessage;
}

export function endProfileSwitch() {
  active.value = false;
  message.value = "";
  targetName.value = null;
}

export function useProfileSwitchUi() {
  return {
    profileSwitchActive: active,
    profileSwitchMessage: message,
    profileSwitchTargetName: targetName,
    beginProfileSwitch,
    setProfileSwitchMessage,
    endProfileSwitch,
  };
}
