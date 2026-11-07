// Two dropdowns backed by the existing single WorkspaceTarget setting.
// Keep positions as numbers and names as strings (including numeric names).
(() => {
  const { streamDeckClient, useSettings } = SDPIComponents;
  const outputSelect = document.getElementById("workspace-output");
  const workspaceSelect = document.getElementById("workspace-position");
  const refreshButton = document.getElementById("refresh-workspaces");

  let outputs = [];
  let workspaces = [];
  let output = "";
  let workspace = "";
  let rendering = false;

  function parseTarget(value) {
    if (!value) return null;
    try {
      const target = JSON.parse(value);
      if (typeof target.output !== "string") return null;
      if (
        typeof target.workspace !== "string" &&
        !(Number.isInteger(target.workspace) && target.workspace >= 1)
      )
        return null;
      return target;
    } catch {
      return null;
    }
  }

  const [getTarget, setTarget] = useSettings(
    "target",
    (value) => {
      const target = parseTarget(value);
      if (target) output = target.output;
      workspace = target ? JSON.stringify(target.workspace) : "";
      render();
    },
    0,
  );

  function option(label, value) {
    const element = document.createElement("option");
    element.textContent = label;
    element.value = value;
    return element;
  }

  function populate(select, choices, selected) {
    const placeholder = option("Choose...", "");
    placeholder.disabled = true;
    select.replaceChildren(placeholder, ...choices);
    select.value = selected;
  }

  function render() {
    rendering = true;
    const outputOptions = outputs.map(({ name }) => option(name, name));
    if (output && !outputs.some((item) => item.name === output)) {
      outputOptions.push(option(`${output} (unavailable)`, output));
    }
    populate(outputSelect, outputOptions, output);

    const available = workspaces.filter((ws) => ws.output === output);
    const indices = [...new Set(available.map((ws) => ws.index))].sort(
      (a, b) => a - b,
    );
    const positions = indices.map((index) =>
      option(String(index), String(index)),
    );
    const names = [
      ...new Set(available.filter((ws) => ws.named).map((ws) => ws.name)),
    ].sort((a, b) => a.localeCompare(b));
    const namedOptions = names.map((name) =>
      option(`${name} (named)`, JSON.stringify(name)),
    );

    // A saved destination survives inventory changes. It becomes selectable
    // again automatically when the workspace reappears and Refresh is clicked.
    if (workspace) {
      const isNamed = workspace.startsWith('"');
      const options = isNamed ? namedOptions : positions;
      if (!options.some((item) => item.value === workspace)) {
        const label = isNamed ? JSON.parse(workspace) : workspace;
        options.push(option(`${label} (unavailable)`, workspace));
      }
    }

    const groups = [];
    if (positions.length) {
      const positionGroup = document.createElement("optgroup");
      positionGroup.label = "Positions";
      positionGroup.append(...positions);
      groups.push(positionGroup);
    }
    if (namedOptions.length) {
      const nameGroup = document.createElement("optgroup");
      nameGroup.label = "Named workspaces";
      nameGroup.append(...namedOptions);
      groups.push(nameGroup);
    }
    populate(workspaceSelect, output ? groups : [], workspace);
    rendering = false;
  }

  outputSelect.addEventListener("valuechange", () => {
    if (rendering || outputSelect.value === output) return;
    output = outputSelect.value;
    workspace = "";
    render();
    setTarget("");
  });

  workspaceSelect.addEventListener("valuechange", () => {
    if (rendering || !output || !workspaceSelect.value) return;
    workspace = workspaceSelect.value;
    setTarget(JSON.stringify({ output, workspace: JSON.parse(workspace) }));
  });

  async function refresh() {
    const message = await streamDeckClient.get(
      "sendToPlugin",
      "sendToPropertyInspector",
      (message) => message.payload?.event === "getWorkspaceDestinations",
      { event: "getWorkspaceDestinations" },
    );
    outputs = message.payload.outputs ?? [];
    workspaces = message.payload.workspaces ?? [];
    render();
  }

  refreshButton.addEventListener("click", refresh);
  getTarget().then((value) => {
    const target = parseTarget(value);
    if (target) {
      output = target.output;
      workspace = JSON.stringify(target.workspace);
    }
    render();
  });
  refresh();
})();
