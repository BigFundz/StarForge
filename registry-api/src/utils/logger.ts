const logger = {
  info: (msg: string, data?: any) => {
    console.log(
      `[INFO] ${new Date().toISOString()} - ${msg}`,
      data ? JSON.stringify(data) : "",
    );
  },
  error: (msg: string, err?: any) => {
    console.error(
      `[ERROR] ${new Date().toISOString()} - ${msg}`,
      err ? JSON.stringify(err) : "",
    );
  },
  warn: (msg: string, data?: any) => {
    console.warn(
      `[WARN] ${new Date().toISOString()} - ${msg}`,
      data ? JSON.stringify(data) : "",
    );
  },
  debug: (msg: string, data?: any) => {
    if (process.env.NODE_ENV === "development") {
      console.debug(
        `[DEBUG] ${new Date().toISOString()} - ${msg}`,
        data ? JSON.stringify(data) : "",
      );
    }
  },
};

export default logger;
