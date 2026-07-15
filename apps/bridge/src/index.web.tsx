import React from "react";
import { AppRegistry } from "react-native";
import App from "./App";

AppRegistry.registerComponent("Bridge", () => App);
AppRegistry.runApplication("Bridge", {
  rootTag: document.getElementById("root"),
});
