<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from '@tauri-apps/api/window';

const greetMsg = ref("");
const name = ref("");

window.onresize = async ()=>{
  await update_window_position();
};

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
  greetMsg.value = await invoke("greet", { name: name.value });
}

async function update_window_position(){
  // const factor = await appWindow.scaleFactor();
  const position = await appWindow.innerPosition();
  const size = await appWindow.innerSize();
  // const logical = position.toLogical(factor);
  let r1 = await invoke("update_window_position", { x: position.x, y: position.y, width: size.width, height: size.height });
  console.log('js执行update_window_position', r1);
}

setInterval(() => {
  update_window_position();
}, 1000);

async function openCamera() {
  await update_window_position();
  let r = await invoke("open_camera", { cameraIndex: 0, widthPercent: .5, heightPercent: .3, offsetXPercent: .0, offsetYPercent: .0 });
  console.log('js执行openCamera OK.', r);
}

async function closeCamera() {
  console.log('js执行closeCamera...');
  let r = await invoke("close_camera");
  console.log('js执行closeCamera OK.', r);
}

</script>

<template>
  <form class="row" @submit.prevent="greet">
    <input id="greet-input" v-model="name" placeholder="Enter a name..." />
    <button type="submit">Greet</button>
  </form>

  <p>{{ greetMsg }}</p>

  <div><button @click="openCamera()">打开相机</button><button @click="closeCamera()">关闭相机</button></div>
</template>
