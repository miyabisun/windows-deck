import "normalize.css";
import "./global.sass";
import { mount } from "svelte";
import App from "./App.svelte";

export default mount(App, { target: document.getElementById("app") });
