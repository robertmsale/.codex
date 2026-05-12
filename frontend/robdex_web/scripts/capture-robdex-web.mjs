import puppeteer from "puppeteer-core";

const output = process.argv[2] ?? `/tmp/robdex-web-pass-${Date.now()}.png`;
const url = process.argv[3] ?? "http://127.0.0.1:42080/";

const browser = await puppeteer.launch({
  executablePath: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  headless: true,
  args: ["--disable-gpu", "--no-first-run"],
});

try {
  const page = await browser.newPage();
  await page.setViewport({ width: 2048, height: 1474, deviceScaleFactor: 1 });
  await page.goto(url, { waitUntil: "networkidle0", timeout: 30000 });
  await page.waitForSelector(".appShell", { timeout: 15000 });
  await page.waitForFunction(
    () => document.querySelectorAll(".chatBubble").length > 0,
    { timeout: 15000 },
  );
  await new Promise((resolve) => setTimeout(resolve, 1000));
  await page.screenshot({ path: output, fullPage: false });
  console.log(output);
} finally {
  await browser.close();
}
