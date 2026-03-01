import Constants from "expo-constants";

const DEV_API_URL = "http://localhost:3000";

export const API_URL =
  Constants.expoConfig?.extra?.apiUrl ?? DEV_API_URL;

export const WS_URL = API_URL.replace(/^http/, "ws");
