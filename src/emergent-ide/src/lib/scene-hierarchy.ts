export type HierarchyNode = {id:string;parent:string|null};

/** Derived authoring traversal; runtime ownership remains in Rust World. */
export function sceneHierarchy<T extends HierarchyNode>(instances:T[]):{instance:T;depth:number}[] {
  const nodes=new Map(instances.map(node=>[node.id,node]));
  if(nodes.size!==instances.length)throw new Error("Scene contains duplicate instance IDs.");
  const children=new Map<string|null,T[]>();
  for(const node of instances){
    if(node.parent!==null&&!nodes.has(node.parent))throw new Error(`Instance '${node.id}' has missing parent '${node.parent}'.`);
    const group=children.get(node.parent)??[];group.push(node);children.set(node.parent,group);
  }
  const stack=(children.get(null)??[]).slice().reverse().map(instance=>({instance,depth:0}));
  const result:{instance:T;depth:number}[]=[];
  while(stack.length){const entry=stack.pop()!;result.push(entry);for(const instance of (children.get(entry.instance.id)??[]).slice().reverse())stack.push({instance,depth:entry.depth+1});}
  if(result.length!==instances.length)throw new Error("Scene parenting would create a hierarchy cycle.");
  return result;
}

export function validParents<T extends HierarchyNode>(instances:T[],id:string):T[] {
  const nodes=new Map(instances.map(node=>[node.id,node]));
  return instances.filter(node=>{let cursor:string|null=node.id;const seen=new Set<string>();while(cursor!==null){if(cursor===id||seen.has(cursor))return false;seen.add(cursor);cursor=nodes.get(cursor)?.parent??null;}return true;});
}
