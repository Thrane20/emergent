const pages = [
  { title: "Introduction", section: "Guide", url: "index.html", terms: "features architecture slotmap overview emergent" },
  { title: "Getting started", section: "Guide", url: "getting-started.html", terms: "install rust run cargo graphics setup" },
  { title: "Hello World", section: "Guide", url: "hello-world.html", terms: "example game behavior entity scene linear motion" },
  { title: "Game projects", section: "Guide", url: "projects.html", terms: "manifest generated code assets output independent crate" },
  { title: "Capability map", section: "Reference", url: "capabilities.html", terms: "roadmap categories graphics audio sound geometry terrain lighting shadows particles input physics animation tools" },
  { title: "Vector geometry", section: "Capabilities", url: "capabilities.html#geometry", terms: "dots points lines polylines polygons vector primitive cube torus pyramid mesh" },
  { title: "Terrain and noise", section: "Capabilities", url: "capabilities.html#terrain", terms: "heightmap polygonal poly terrain perlin simplex worley procedural generation" },
  { title: "Lighting and effects", section: "Capabilities", url: "capabilities.html#look", terms: "lights shadows particles materials textures animation post processing" },
  { title: "Audio", section: "Capabilities", url: "capabilities.html#audio", terms: "sound mixer clips stream music spatial listener attenuation doppler" },
  { title: "Simulation services", section: "Capabilities", url: "capabilities.html#simulation", terms: "input joystick gamepad physics collision navigation ui" },
  { title: "Library and API", section: "Reference", url: "api.html", terms: "engine world methods structs enums reference" },
  { title: "Engine lifecycle", section: "API", url: "api.html#engine", terms: "Engine Game EngineConfig run run_ticks advance_frame" },
  { title: "World and entities", section: "API", url: "api.html#world", terms: "World EntityId spawn child subtree hierarchy" },
  { title: "Transforms", section: "API", url: "api.html#transforms", terms: "Transform GlobalTransform Vec3 Quat reparent propagation" },
  { title: "Behaviours", section: "API", url: "api.html#behaviours", terms: "BehaviorSpec link port linear motion follow scalar snapshot" },
  { title: "ECS integration", section: "API", url: "api.html#ecs", terms: "bevy component resource schedule lifecycle queue" },
  { title: "Graphics and geometry", section: "API", url: "api.html#graphics", terms: "wgpu winit GeometryRenderer GeometryId GeometryInstance Point Line Polygon Mesh render resize" },
  { title: "Project contract", section: "API", url: "api.html#projects", terms: "Project ProjectManifest output asset path schema" },
];

const root = document.documentElement.dataset.root || ".";
const resolve = (url) => `${root}/${url}`.replace("././", "./");
const dialog = document.querySelector("#search-dialog");
const input = document.querySelector("#search-input");
const results = document.querySelector("#search-results");

function renderResults(query = "") {
  const words = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const matches = pages.filter((page) => {
    const haystack = `${page.title} ${page.section} ${page.terms}`.toLowerCase();
    return words.every((word) => haystack.includes(word));
  });
  results.innerHTML = matches.length
    ? matches.map((page) => `
      <a class="search-result" href="${resolve(page.url)}">
        <strong>${page.title}</strong><span>${page.section}</span>
      </a>`).join("")
    : '<div class="search-empty">No guide pages match that search.</div>';
}

function openSearch() {
  renderResults();
  dialog.showModal();
  requestAnimationFrame(() => input.focus());
}

document.querySelectorAll("[data-search]").forEach((button) => button.addEventListener("click", openSearch));
document.querySelector("#search-close")?.addEventListener("click", () => dialog.close());
input?.addEventListener("input", (event) => renderResults(event.target.value));
dialog?.addEventListener("click", (event) => {
  if (event.target === dialog) dialog.close();
});
document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    dialog.open ? dialog.close() : openSearch();
  }
});

document.querySelector("#menu-trigger")?.addEventListener("click", () => {
  document.body.classList.toggle("nav-open");
});
document.querySelectorAll(".sidebar a").forEach((link) => link.addEventListener("click", () => {
  document.body.classList.remove("nav-open");
}));

document.querySelectorAll("pre").forEach((pre) => {
  const block = pre.closest(".code-block");
  if (!block) return;
  const button = document.createElement("button");
  button.className = "copy-button";
  button.type = "button";
  button.textContent = "Copy";
  button.addEventListener("click", async () => {
    await navigator.clipboard.writeText(pre.textContent);
    button.textContent = "Copied";
    window.setTimeout(() => { button.textContent = "Copy"; }, 1400);
  });
  block.appendChild(button);
});
