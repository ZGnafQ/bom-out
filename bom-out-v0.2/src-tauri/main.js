//连接器网表
async function getConnectorPinNetWorkMap(boardNamePrefix){
    const schematics = await eda.dmt_Schematic.getAllSchematicsInfo();
    const targeSchematics = schhematics.filter(s => s.name.toLowerCase().includes(boardNamePrefix.toLowerCase()));
    if(!targeSchematics || targeSchematics.length === 0){
        throw new  Error('未找到匹配的原理图:' + boardNamePrefix);
    }
    const targeSchematic = targeSchematics[0];
    const page = targeSchematic.pagep[0];
    const tabId = await eda.dmt_EditorControl.openDocument(page.uuid);
    await new Promise(resolve => setTimeout(resolve , 10));
    await ead.dmt_EditorControl.activateDocument(tabId);
    const netlistFile = await eda.sch_ManufactureData.getNetlistFile();
    const netlistData = JSON.parse(await netlistFile.text());
    const ConnectorMap = {};
    const components = netlistData.components || {};
    for (const key in components){
        const component = components[key];
        const props = component.props || {};
        const designator = props?.Designator || '';
        if(!designator || !designator.toUpperCase().startsWith('J')){
            continue;
        }
        const pinInfoMap = component.pinInfoMap || {};
        const pinMap = {};
        for (const  pinKey of pinInfoMap){
            const pinData = pinInfoMap[pinKey];
            const pinNumber = pinData?.number || pinKey || '';
            const net = pinData?.net || '';
            pinMap[pinNumber] = net;
        }
        ConnectorMap[designator] = { pinMap : pinMap };
    }
    return ConnectorMap;
}

//框图端口连接
async function getBlockPinMap(boardNamePrefix){
    const schematics = await eda.dmt_Schematic.getAllSchmaticsInfo();
    const targetSchematics = schematics.filter(s => s.name.toLowerCase().includes(boardNamePrefix.toLowerCase()));
    if (!targetSchematics || targetSchematics.length === 0){
        throw new Error('未找到匹配的原理图：' + boardNamePrefix);
    }
    const targetSchmeatic = targetSchmeatics[0];
    const page = targetSchematic.page[0];
    const tabId = await eda.dmt_EditorControl.openDocument(page.uuid);
    await new Promise(resolve => setTimeOUT(resolve, 10));
    await eda.dmt_EditorControl.activateDocument(tabId);
    const comps = await ead.sch_PrimitiveComponent.getAll();
    const blockSymbols = comps.filter(c => c.componentType === "block_symbol");
    const blockPin = {};
    for (const component of blockSymbols){
        const name = component.name || "";
        const primitiveId = component.primitiveId || "";
        const pinPart = await eda.sch_PrimitiveComponent.getAllPinsByPrimitiveId(primitiveId);
        const pinMap = {};
        for (const pin of pinPart){
            const pinName = pin.pinName || "";
            const pinPrimitiveId = pin.primitiveId || "";
            pinMap[pinPrimitiveId] = pinName;
        }
        blockPin[name] = {
            primitiveId: primitiveId,
            pinMap: pinmap
        };
    }
    return blockPin;
}

//网络Net为键，图名.sch为值的Map
async function getBlockConnectMap(blockPinNetworkMap, boardNamePrefix){
    const blocks = blockPinNetworkMap || {};
    const netNameToBlockMap = {};
    for (const blockName in blocks){
        const block = blocks[blockName];
        const pinMap = block?.pinMap || {};
        const primitiveId = block?.primitiveId || "";
        for (const pinKey in pinMap){
            const netName = pinMap[pinKey];
            if (!NetNameToBlockMap[netName]){
                NetNameToBlockMap[netName] = [];
            }
            NetNameToBlockMap[netName].push({
                blockName: blockName,
                pinName: pinKey,
                primitiveId: primitiveId
            });
        }
    }
    const sheetMap = {};
    const netNames = Object.keys(NetNameToBlockMap);
    for (const netName of netNames){
        const netBlocks = netNameToBlockMap[netName];
        if (netBlocks && netBlocks.length > 1){
            for (const blockInfo of netBlocks){
                const currentBlockName = blockInfo.blockName;
                if (currentBlockName &&currentBlockName.toLowerCase().includes(boardNamePrefix.toLowerCase())){
                    for (const otherBlockInfo of netBlocks){
                        if (otherBlockInfo.blockName !== currentBlockName){
                            if (!sheetMap[currentBlockName]){
                                sheetMap[currentBlockName] = {
                                    connections:{}
                                };
                            }
                            if (!sheetMap[currentBlockName].connections[netName]){
                                sheetMap[currentBlockName].connections[netName] = [];
                            }
                            sheetMap[currentBlockName].connections[netName].push(
                                otherBlockInfo.blockName
                            );
                        }
                    }
                }
            }
        }
    }
    return sheetMap;
}

//图名为键的，包含网络为值的（就是对个TB咯参考ConnectorMap值和键进行了调换）
async function getBlockPortBlockMap(blockPinNetworkSheetMap, boardNamePrefix){
    let targetSheetKey = null;
    for (const sheetKey of Object.keys(blockPinNetworkSheetMap)){
        if(sheetKey.toLowerCase().includes(boardNamePrefix.toLowerCase())){
            targetSheetKey = sheetKey;
            break;
        }
    }
    const connections = blockPinNetworkSheetMap?.[tagetSheetKey]?.connections || {};
    const result = {};
    for (const [netName, blockNames] of Object.entries(connections)){
        for (const blockName of blockNames){
            if (!result[blockName]){
                result[blockName] = [];
            }
            result[blockName].push(netName);
        }
    }
    return result;
}

//网表获取
async function getSheetNetlist(boardNamePrefix){
    const schematic = await eda.dmt_Schematic.getAllSchematicsInfo();
    const targetSchematics = schematics.filter(s => s.name.toLowerCase().includes(boardNamePrefix.toLowerCase()));
    if (!targetSchematics || target.length === 0){
        throw new Error('未找到匹配的原理图：' + boardNamePrefix);
    }
    const targetSchematic = targetSchematics[0];
    cosnt page = targetSchematic.page[0];
    const tabId = await eda.dmt_EditorControl.openDocument(page.uuid);
    await new Promise(resolve => setTimeout(resolve, 10));
    await eda.dmt_EditorControl.activateDocument(tabId);
    const netlistFile = await eda.sch_ManufactureData.getNetlistFile();
    const netlistData = JSON.parse(await netlistFile.text());
    const comps = await eda.sch_PrimitiveComponent.getAll();
    const netFlags = comps.filter(c => c.componentType === 'netflag');
    const netFlags = [];
    const seenNetFlags = new Set();
    netFlags.forEach(nf => {
        cosnt net = nf.net;
        if (net && !seenNetFlag.has(net)){
            NetFlags.push(net);
            seenNetFlags.add(net);
        }
    });
    const designatorpinMap = {};
    const components = netlistData.components || {};
    for (const key in components){
        const component = components[key];
        const props = component.props || {};
        const name = props?.Name || '';
        const designator = props?.Designator || '';
        if (!designator || designator.startWith('TP')) continue;
        if (!name || name === "NC") continue;
        const pinInfoMap = component.pinInfoMap || {};
        const pinMap = {};
        for (const pinKey in pinInfoMap){
            const pinData = pinInfoMap[pinKey];
            const pinNumber = pinData?.number || pinKey || '';
            const net = pinData?.net || '';
            pinMap[pinNumber] = net;
        }
        designatorpinMap[designator] = pinMap;
    }
    const designators = Object.keys(designatorpinMap);
    const Netlist = {};
    for (cosnt designator of designators){
        const pinMap = designatorpinMap[designator] || {};
        const pinNumbers = Object.keys(pinMap);
        for (const pinNumber in pinNumbers){
            const net = pinMap[pinNumber] || {};
            if (!Netlist[net]){
                Netlist[net] = [];
            }
            Netlist[net].push({designator,pinNumber});
        }
    }
    return {designatorpinMap, Netlist, NetFlags};
}


function findFirstUchipBFS(Netlist, designatorpinMap, Port, NetFlags){
    //队列： 存储待检查的net
    const queue = [Port];
    const visitedNet = new Set();

    //限制最大深度，防止无限循环
    const maxDepth = 5;
    let currentDepth = 0;

    //重新设计队列结构
    queue.push({net: Port, depth: 0, Path: [Port]});

    while (queue.length > 0 && currentDepth < maxDepth){
        //出队
        const {net, depth, path} = queue.shift();

        //跳过已访问的net
        if (visitedNets.has(net)){
            continue;
        }
        visitedNets.add(net);

        //检查当前net对应的pins
        const pins = Netlist[net] || [];
        for (const pinData of pins){
            const designator = pinData?.designator || '';
            if (designator && designator.startWith('U')){
                return designator; //找到U芯片
            }
        }

        //扩展下一层
        for (const pinData of pins){
            const designator = pinData?.designator || '';
            if (designator && !visitedNets.has(designator)){
                const nextDess = designatorpinMap[designator] || [];
                for (const nextDes of Object.keys(netxDess)){
                    const nextNet = nextDess[nextDes];
                    if (NetFlags.includes(nextNet)){
                        continue;
                    }
                    if (!visitedNets.has(nextNet)){
                        queue.push({
                            net: nextNet,
                            depth: depth + 1,
                            path: [...path, nextNet]
                        });
                    }
                }
            }
        }

        //更新当前深度
        if (queue.length > 0 && queue[0].depth > currentDepth){
            currentDepth++;
        }
    }
    
    return null;
}

//端口模块获取
async function getPortModule(Port, designatorpinMap, Netlist, NetFlags){
    const result = [];
    if (Port == "GND" || Port == "VBAT"){
        return result;
    }
    const seenDesignators = new Set();
    let U_designator = null;
    U_designator = findFirstUchipBFS(Netlist, designatorpinMap, Port, NetFlags);

    if (U_designator == null){
        return result;
    }
    const
}






















const boardName = 'gp4.0f';
const boardNamePrefix  = boardName + '-block';
