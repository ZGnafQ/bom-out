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
    for (const key of components){
        const component = components[key];
        const props = component.props || {};
        const name = props?.Name || '';
        const designator = props?.Designator || '';
        if (!designator || designator.startWith('TP')) continue;
        if (!name || name === "NC") continue;
        const pinInfoMap = component.pinInfoMap || {};
        const pinMap = {};
        for (const pinKey of pinInfoMap){
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
    const U_designatorpinMap = designatorpinMap[U_designator];
    const pinNumbers = Object.keys(U_designatorpinMap);
    for (const pinNumber of pinNumbers){
        const netName = U_designatorpinMap[pinNumber];
        if (NetFlags.includes(NetName)){
            continue;
        }
        const pins = Netlist[netName] || [];
        for (const pinData of pins){
            const d = pinData?.designator || '';
            if (d.startWith('U') && !(d === U_designator)){
                continue;
            }
            if (d && !seenDesignators.has(d)){
                result.push(d);
                seenDesignators.add(d);
            }
        }
    }
    return result;
}

async function BFS(result, designatorpinMap, NetFlags, Netlist){
    let queue = Array.isArray(result) ? result : [];
    let satisfied = [];
    const seenSatisfied = new Set();
    let item = queue.shift();
    while (item && !item.startWith('U')){
        queue.push(item);
        item = queue.shift();
    }
    const U_chip = item;
    if (item && !seenSatisfied.has(item)){
        satisfied.push(item);
        seenSatisfied.add(item);
    }
    while (queue.length > 0){
        item = queue.shift();
        const pin_net = designatorpinMap[item];
        for (const pin of Object.keys(pin_net)){
            const net = pin_net[pin];
            if (NetFlags.includes(net)){
                continue;
            }
            const designators = Netlist[net];
            let netPassed = true;
            if (!Array.isArray(designators)){
                continue;
            }
            for (const designator_data of designators){
                const new_des = designator_data.designator;
                if (!(new_des === U_chip) && new_des.startWith('U')){
                    continue;
                }
                if (satisfied.includes(new_des)){
                    continue;
                }
                else{
                    queue.push(new_des);
                    if (item && !seenSatisfied.has(item)){
                        satisfied.push(item);
                        seenSatisfied.add(item);
                    }
                    netPassed = false;
                }
            }
        }
    }
    return satisfied;
}
//这边下面好像可以不用把getSheetNetlist, getPortModule这两个函数当做参数传入
async function connectorPortUse(blockPortBlock, getSheetNetlist, getPortModule){
    const sheetMap_total = {};
    const boardNamePrefix = Object.keys(blockPortBlock);
    for (const boardNamePrefix_order of boardNamePrefix){
        const {designatorpinMap, Netlist, NetFlags} = await getSheetNetlist(boardNamePrefix_order);
        const Ports = blockPortBlock[boardNamePrefix_order];
        for (const Port of Ports){
            const result = await getPortModule(Port, designatorpinMap, Netlist, NetFlags);
            const sheetMap = await BFS(result, designatorpinMap, NetFlags, Netlist);
            // Merge objects
            if (sheetMap_total[Port]){
                sheetMap_total[Port].push(...sheetMap);
            } else {
                sheetMap_total[Port] = sheetMap;
            }
        }
    }
    return sheetMap_total;
}

async function getConnectorModule(ConnectorMap, SheetMap_total){
    const ConnectorModule_total = {};
    const ConnectorNames = Object.keys(ConnectorMap);
    const pinMap = 'pinMap';
    for (const ConnectorName of ConnectorNames){
        const pinmap = ConnectorMap[ConnectorName][pinMap];
        const pinNumbers = Object.keys(pinmap);
        for (const pinNumber of pinNumbers){
            const Net = pinmap[pinNumber];
            const ConnectorRename = ConnectorName.slice(1) + pinNumber;
            ConnnectorModule_total[ConnectorRename] = sheetMap_total[Net];
        }
    }
    return ConnectorModule_total;
}


// 调用函数
// 读取并解析JSON文件
const boardNameResponse = await fetch('http://localhost:49620/read-file?path=d:/output/board_name.json');
const boardNameData = await boardNameResponse.json();
const boardName = JSON.parse(boardNameData.content).board_name; // "gp4.0f"
const boardNamePrefix = boardName + '-连接器'；
const connectorPinNetworkMap = await getConnectorPinNetworkMap(boardNamePrefix);

const boardNamePrefix_block = boardName + '-block';
const blockPin = await getBlockPinMap(boardNamePrefix_block);
const blockConnector = await getBlockConnectorMap(blockPin, boardNamePrefix);
const blockPortBlock = await getBlockPortBlockMap(blockConnector, boardNamePrefix);


const boardNamePrefix_Hbridge = boardName + '-低边驱动';
const {designatorpinMap, Netlist, NetFlags} = await getSheetNetlist(boardNamePrefix_Hbridge);
const Port = "P-LSO14";
const result_1 await getPortModule(Port, designatorpinMap, Netlist, NetFlags);
const sheetMap = await BFS(result_1, designatorpinMap, NetFlags, Netlist);
cosnt sheetMap_total = await connectorPotUse(blockPortBlock, getSheetNetlist, getPortModule);
const ConnectorModule_total = await getConnectorModule(connectorPinNetworkMap, sheetMap_total);

//数据保存
const jsonData = {
    // connectorMap: connectorPinNetworkMap,
    // blockPin: blockPin,
    // blockConnector: blockConnector,
    // blockPortBlock: blockPortBlock,
    // designatorpinMap: designatorpinMap,
    // Netlist: Netlist,
    // NetFlags: NetFlags,
    // sheetMap: sheetMap,
    // sheetMap_total: sheetMap_total,
    // generatedAt: new Date().toISOString()
    ConnectorModule_total:ConnectorModule_total
    // boardname: boardName
};
const fileName = boardName + '.json';
const jsonStr = JSON.stringify(jsonData, null, 2);
const outputPath = './output/' + fileName;
//const outputPath = '../output/' +fileName; 这样是bridge-server.mjs的父级目录 最后反正用./就行了
const response = await fetch('http://localhost:49620/save-file',{
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify({filePath: outputPath, Content: jsonStr})
});
const result = await response.json();

















const boardName = 'gp4.0f';
const boardNamePrefix  = boardName + '-block';
