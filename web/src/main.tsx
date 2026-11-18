import { render } from "@solidjs/web";
import "@fontsource-variable/nunito-sans";
import "maplibre-gl/dist/maplibre-gl.css";
import "./styles.css";
import { App } from "./App";

render(() => <App />, document.getElementById("root")!);
