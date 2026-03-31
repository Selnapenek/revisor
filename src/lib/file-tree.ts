import type { ProcessedFile } from "~/api/files";

export interface FileNode {
  type: "file";
  name: string;
  path: string;
  file: ProcessedFile;
}

export interface DirNode {
  type: "dir";
  name: string;
  path: string;
  children: TreeNode[];
}

export type TreeNode = FileNode | DirNode;

/**
 * Build a hierarchical tree from a flat list of files.
 * Collapses single-child directories (e.g. `client/src` becomes one node).
 * Sorts directories first, then files, alphabetically within each group.
 */
export function buildFileTree(files: ProcessedFile[]): TreeNode[] {
  // Build raw tree
  const root: DirNode = { type: "dir", name: "", path: "", children: [] };

  for (const file of files) {
    const parts = file.filename.split("/");
    let current = root;

    for (let i = 0; i < parts.length - 1; i++) {
      const dirName = parts[i];
      const dirPath = parts.slice(0, i + 1).join("/");
      let child = current.children.find(
        (c): c is DirNode => c.type === "dir" && c.path === dirPath,
      );
      if (!child) {
        child = { type: "dir", name: dirName, path: dirPath, children: [] };
        current.children.push(child);
      }
      current = child;
    }

    current.children.push({
      type: "file",
      name: parts[parts.length - 1],
      path: file.filename,
      file,
    });
  }

  // Collapse single-child directories and sort
  return collapseAndSort(root.children);
}

function collapseAndSort(nodes: TreeNode[]): TreeNode[] {
  return nodes
    .map((node) => {
      if (node.type === "file") return node;

      // Recursively process children first
      node.children = collapseAndSort(node.children);

      // Collapse: if dir has exactly one child and it's a dir, merge them
      while (
        node.children.length === 1 &&
        node.children[0].type === "dir"
      ) {
        const child = node.children[0] as DirNode;
        node = {
          type: "dir",
          name: `${node.name}/${child.name}`,
          path: child.path,
          children: child.children,
        };
      }

      return node;
    })
    .sort((a, b) => {
      // Dirs first, then files
      if (a.type !== b.type) return a.type === "dir" ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
}
